//! Resolve checked-local storage against its current SSA value or primitive place.
use super::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees,
    LoweringError, PlaceId, PrimitiveType, ScalarType, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeId, terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::expression_preparation::source_custody;

#[cfg(test)]
mod case_observation_tests;
pub(crate) mod recast_records;
pub(crate) mod structural_cases;
pub(crate) mod structural_fields;
pub(crate) mod structural_paths;
#[cfg(test)]
mod tests;
pub(crate) mod view_locals;
pub(crate) use structural_fields::{StructuralFieldBindings, StructuralScalarFieldBinding};

/// The access of a readable loan a view or borrowed selection joins: a
/// shared read or an exclusive borrow. A write-only loan lends no readable
/// place, so it has no variant here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoanAccess {
    Shared,
    Mutable,
}

impl LoanAccess {
    /// The access the checked plan records for this loan.
    pub(crate) fn planned_access(self) -> checked_trees::CheckedStructuralAccess {
        match self {
            Self::Shared => checked_trees::CheckedStructuralAccess::SharedBorrow,
            Self::Mutable => checked_trees::CheckedStructuralAccess::MutableBorrow,
        }
    }

    /// The access the Terminal structural argument carries for this loan.
    pub(crate) fn structural_access(self) -> StructuralAccess {
        match self {
            Self::Shared => StructuralAccess::SharedBorrow,
            Self::Mutable => StructuralAccess::MutableBorrow,
        }
    }
}

/// The exact published local carrier stays with its source name and custody.
/// A reference result may retain its reference shell even when its argument
/// access is shared; access alone therefore cannot recover this type.
#[derive(Clone, Debug)]
pub(crate) struct StructuralLocalBinding {
    pub(crate) symbol: symbols::SymbolHandle,
    pub(crate) argument: StructuralArgument,
    pub(crate) structural_type: StructuralTypeId,
}

#[derive(Clone)]
pub(crate) struct ScalarBindings {
    /// The binding-ordinal namespace: the scalar formals, then every
    /// established scalar result in statement order. A retained call result
    /// that feeds an assignment occupies a slot here like an immutable local,
    /// because checked result bindings number both shapes densely.
    immutable: Vec<Option<usize>>,
    /// The authored namespace a checked `Parameter`/`Local` read names: the
    /// scalar formals, then the immutable locals the source declares, in
    /// order. A call result stored straight into a field has no source name,
    /// so it never occupies a slot here; resolving authored reads through the
    /// binding namespace instead would shift every later local onto the
    /// preceding call's result.
    authored: Vec<Option<(usize, ScalarType)>>,
    parameter_count: usize,
    storage: Vec<(symbols::SymbolHandle, ScalarType, usize)>,
    primitive_storage: Vec<(symbols::SymbolHandle, PlaceId, ScalarType)>,
    /// Authored parameter positions stay separate from dense Terminal positions.
    structural_parameters: Vec<(u32, StructuralParameterDeclaration)>,
    structural_locals: Vec<StructuralLocalBinding>,
    local_cases: Vec<structural_cases::LocalCaseBinding>,
    structural_fields: StructuralFieldBindings,
    structural_cases: Vec<structural_cases::StructuralCaseBinding>,
    /// Whole element-view parameters resolved to their scalar element type,
    /// absent for record elements (which have a length but no scalar read).
    /// `&[u8]` stays on the byte path.
    element_views: std::collections::BTreeMap<StructuralTypeId, Option<ScalarType>>,
    /// Established immutable view locals an observation may root at.
    view_locals: Vec<view_locals::ViewLocalBinding>,
    byte_record_loans:
        std::sync::Arc<[crate::expression_preparation::bindings::recast_records::ByteRecordLoan]>,
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
            .position(
                |StructuralLocalBinding {
                     symbol: candidate,
                     argument,
                     ..
                 }| {
                    *candidate == symbol
                        && argument.place == place
                        && argument.access == StructuralAccess::Owned
                        && argument.path.is_empty()
                },
            )
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
        structural_type: StructuralTypeId,
    ) -> Result<(), LoweringError> {
        if !symbol.is_valid()
            || self
                .structural_locals
                .iter()
                .any(|local| local.symbol == symbol)
        {
            return unsupported("structural local has invalid or repeated identity");
        }
        self.structural_locals.push(StructuralLocalBinding {
            symbol,
            structural_type,
            argument: StructuralArgument {
                place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
        });
        Ok(())
    }

    /// The place half of `shared_structural_argument`: the custody checks and
    /// source-to-place resolution, without the path lower. Callers whose
    /// checked path carries a `RuntimeIndex` lower it through
    /// `structural_paths::lower_argument_path` instead of `lower_structural_path`.
    pub(crate) fn shared_structural_place(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        machine: &checked_trees::machine::Machine,
        parameters: &[checked_trees::signature::StateParameter],
    ) -> Result<PlaceId, LoweringError> {
        if argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow {
            return unsupported("computed shared argument changes its access");
        }
        self.borrowed_structural_place(argument, machine, parameters)
    }

    /// Resolve an established home under the requested loan. Source replay owns
    /// the authored permission; this join independently bounds it by the access
    /// retained on the emitted storage itself.
    pub(crate) fn borrowed_structural_place(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        machine: &checked_trees::machine::Machine,
        parameters: &[checked_trees::signature::StateParameter],
    ) -> Result<PlaceId, LoweringError> {
        if !matches!(
            argument.access,
            checked_trees::CheckedStructuralAccess::SharedBorrow
                | checked_trees::CheckedStructuralAccess::MutableBorrow
        ) {
            return unsupported("computed argument requires readable borrowed access");
        }
        let supports_borrow = |source| {
            matches!(
                source,
                StructuralAccess::Owned | StructuralAccess::MutableBorrow
            ) || (source == StructuralAccess::SharedBorrow
                && argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow)
        };
        // A shared read retains the established place and its qualifications;
        // it cannot invalidate them. Mutable borrowing still needs a separate
        // preservation contract, and projected qualification custody is not
        // represented by this join.
        let supports_parameter = |source: &StructuralParameterDeclaration| {
            supports_borrow(source.access)
                && (source.qualifications.is_empty()
                    || argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow)
                && source.projected_qualifications.is_empty()
        };
        let place = match argument.source {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
            | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                root: checked_trees::CheckedStorageRoot::OwnedLocal { symbol },
                ..
            }
            | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                root: checked_trees::CheckedStorageRoot::OwnedLocal { symbol },
                ..
            } => {
                let mut locals = self
                    .structural_locals
                    .iter()
                    .filter(|row| row.symbol == symbol);
                match locals.next() {
                    Some(source) => {
                        // An owned local lends its whole place; a `&T` local
                        // is itself a shared-borrow join result, so borrowing
                        // through it reuses that same shared custody rather
                        // than fabricating ownership.
                        if !symbol.is_valid()
                            || locals.next().is_some()
                            || !supports_borrow(source.argument.access)
                            || !source.argument.path.is_empty()
                        {
                            return unsupported("computed borrow changes its local custody");
                        }
                        source.argument.place
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
                                "computed borrow lost its established local",
                            ))?;
                        let (_, source) = self
                            .structural_parameters
                            .iter()
                            .find(|(place_position, _)| *place_position as usize == position)
                            .ok_or(LoweringError::Unsupported(
                                "computed borrow lost its parameter",
                            ))?;
                        if !supports_parameter(source) {
                            return unsupported("computed borrow widens its parameter custody");
                        }
                        source.place
                    }
                }
            }
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index,
            }
            | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                root:
                    checked_trees::CheckedStorageRoot::Parameter {
                        index: parameter_index,
                    },
                ..
            }
            | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                root:
                    checked_trees::CheckedStorageRoot::Parameter {
                        index: parameter_index,
                    },
                ..
            } => {
                let (_, source) = self
                    .structural_parameters
                    .get(parameter_index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "computed borrow lost its parameter",
                    ))?;
                if !supports_parameter(source) {
                    return unsupported("computed borrow widens its parameter custody");
                }
                source.place
            }
            _ => return unsupported("computed borrow has no established source"),
        };
        Ok(place)
    }

    /// Resolve the exact published carrier at a live source place.
    pub(crate) fn structural_place_type(&self, place: PlaceId) -> Option<StructuralTypeId> {
        let mut types = self
            .structural_parameters
            .iter()
            .filter(|(_, source)| source.place == place)
            .map(|(_, source)| source.structural_type)
            .chain(
                self.structural_locals
                    .iter()
                    .filter(|local| local.argument.place == place)
                    .map(|local| local.structural_type),
            );
        let structural_type = types.next()?;
        types
            .all(|candidate| candidate == structural_type)
            .then_some(structural_type)
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
        self.borrowed_structural_argument(argument, machine, parameters)
    }

    /// Bind an exact shared or mutable loan to its existing structural home.
    pub(crate) fn borrowed_structural_argument(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        machine: &checked_trees::machine::Machine,
        parameters: &[checked_trees::signature::StateParameter],
    ) -> Result<StructuralArgument, LoweringError> {
        // Not `terminal_access`: only a shared or mutable loan binds here.
        let access = match argument.access {
            checked_trees::CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
            checked_trees::CheckedStructuralAccess::MutableBorrow => {
                StructuralAccess::MutableBorrow
            }
            _ => return unsupported("computed argument requires readable borrowed access"),
        };
        Ok(StructuralArgument {
            place: self.borrowed_structural_place(argument, machine, parameters)?,
            path: structural_paths::lower_structural_path(&argument.path)?,
            access,
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
            .filter(|local| local.symbol == symbol);
        let StructuralLocalBinding {
            argument: source, ..
        } = matches.next().ok_or(LoweringError::Unsupported(
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

    /// Rejoin an authored formal symbol only after receiver retention has
    /// settled the structural roster. Actual locals keep their separate case
    /// bindings; a missing formal cannot silently become a local observation.
    pub(crate) fn case_parameter_argument(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        machine: &checked_trees::machine::Machine,
        parameters: &[checked_trees::signature::StateParameter],
    ) -> Result<Option<checked_trees::CheckedUnitStructuralArgumentPlan>, LoweringError> {
        match argument.source {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { .. } => {
                return Ok(Some(argument.clone()));
            }
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                if !symbol.is_valid() {
                    return unsupported("case observation has an invalid source symbol");
                }
                let mut formals = parameters.iter().filter(|parameter| {
                    parameter.symbol == symbol || (parameter.is_self && symbol == machine.symbol)
                });
                if formals.next().is_none() {
                    return Ok(None);
                }
                if formals.next().is_some()
                    || self
                        .structural_locals
                        .iter()
                        .any(|local| local.symbol == symbol)
                {
                    return unsupported("case observation has ambiguous formal custody");
                }
            }
            _ => return unsupported("case observation has no established source"),
        }
        let source = self.shared_structural_place(argument, machine, parameters)?;
        let mut matching = self
            .structural_parameters
            .iter()
            .enumerate()
            .filter(|(_, (_, parameter))| parameter.place == source);
        let (position, _) = matching.next().ok_or(LoweringError::Unsupported(
            "case observation lost its retained formal",
        ))?;
        if matching.next().is_some() {
            return unsupported("case observation has ambiguous retained formals");
        }
        let mut resolved = argument.clone();
        resolved.source = checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(position).map_err(|_| {
                LoweringError::Unsupported("case observation formal index exceeds its namespace")
            })?,
        };
        Ok(Some(resolved))
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

    /// A case observation of a runtime-selected element of a parameter's
    /// fixed array or element view. The checked path
    /// carries exactly one `RuntimeIndex`; its selector is read first
    /// and becomes the observation's index, and the static halves on either
    /// side resolve against the parameter's declarations. A static path keeps
    /// `parameter_case_observation`.
    pub(crate) fn parameter_element_case_observation(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        case: &str,
        scalar_types: &[semantic_vocabulary::QualifiedScalarType],
    ) -> Result<LoweredBooleanReturnExpression, LoweringError> {
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
        let mut runtime = argument.path.iter().enumerate().filter(|(_, segment)| {
            matches!(
                segment,
                checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(_)
            )
        });
        let Some((
            selector_position,
            checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(selector),
        )) = runtime.next()
        else {
            return unsupported("runtime case observation has no runtime element");
        };
        if runtime.next().is_some() {
            return unsupported("runtime case observation selects more than one element");
        }
        let static_path = |segments: &[checked_trees::CheckedUnitStructuralPathSegment]| {
            segments
                .iter()
                .map(|segment| match segment {
                    checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => {
                        Ok(checked_trees::CheckedStructuralPredicatePathSegment::Field(
                            identity.clone(),
                        ))
                    }
                    checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) => Ok(
                        checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(*index),
                    ),
                    _ => unsupported("runtime case observation has a non-field projection"),
                })
                .collect::<Result<Vec<_>, _>>()
        };
        let (source, path, element_path, case, observes_element_view_length) =
            structural_cases::resolve_runtime_element(
                &self.structural_cases,
                *parameter_position,
                &static_path(&argument.path[..selector_position])?,
                &static_path(&argument.path[selector_position + 1..])?,
                case,
            )?;
        let index = match selector {
            checked_trees::CheckedRuntimeIndex::Parameter { position } => {
                self.runtime_index_parameter(*position)?
            }
            checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
                let (selector_source, selector_path, leaf, scalar_type) =
                    structural_fields::resolve_typed(
                        &self.structural_fields,
                        field.parameter_position,
                        &field.path,
                    )?;
                LoweredDirectExpression::StructuralField {
                    source: selector_source,
                    path: selector_path,
                    field: leaf,
                    scalar_type,
                }
            }
            // A hoisted computed selector is an established immutable local:
            // the observation reads its value at the local's dense position.
            checked_trees::CheckedRuntimeIndex::Local { position } => {
                let position = self.authored_position(*position as usize)?;
                LoweredDirectExpression::Local {
                    position,
                    scalar_type: scalar_types
                        .get(position)
                        .ok_or(LoweringError::Unsupported(
                            "runtime case observation selector is outside its scalar frame",
                        ))?
                        .scalar_type,
                }
            }
            _ => return unsupported("runtime case observation requires a bound scalar selector"),
        };
        if !matches!(
            index.scalar_type(),
            ScalarType::Integer(integer) if !integer.is_address()
        ) {
            return unsupported("runtime case observation selector is not an integer read");
        }
        Ok(LoweredBooleanReturnExpression::IndexedCaseMembership {
            source,
            observes_element_view_length,
            path,
            index: Box::new(index),
            element_path,
            case,
        })
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
        self.element_views = element_views(&self.structural_parameters, types);
        self
    }

    /// Adopt the element views an evaluation resolved for its parameters, so
    /// a namespace built without its own observations still reads and
    /// measures an element-view parameter with element operations.
    pub(crate) fn with_element_views(
        mut self,
        views: &std::collections::BTreeMap<StructuralTypeId, Option<ScalarType>>,
    ) -> Self {
        self.element_views
            .extend(views.iter().map(|(view, element)| (*view, *element)));
        self
    }

    pub(crate) fn with_resolved_structural_observations(
        mut self,
        fields: &StructuralFieldBindings,
        cases: &[structural_cases::StructuralCaseBinding],
    ) -> Self {
        self.structural_fields = fields.clone();
        self.structural_cases = cases.to_vec();
        self
    }

    /// Guard lowering keeps case-qualified payload reads for the decision
    /// lowering whose case dispatch binds them; every other context refuses.
    pub(crate) fn with_deferred_case_payloads(mut self) -> Self {
        structural_fields::defer_case_payloads(&mut self.structural_fields);
        self
    }

    pub(crate) fn for_computation_operands(
        offset: usize,
        types: impl IntoIterator<Item = ScalarType>,
    ) -> Self {
        let authored = types
            .into_iter()
            .enumerate()
            .map(|(position, scalar_type)| Some((offset + position, scalar_type)))
            .collect::<Vec<_>>();
        Self {
            immutable: authored
                .iter()
                .map(|binding| binding.map(|row| row.0))
                .collect(),
            parameter_count: authored.len(),
            authored,
            storage: Vec::new(),
            primitive_storage: Vec::new(),
            structural_parameters: Vec::new(),
            structural_locals: Vec::new(),
            local_cases: Vec::new(),
            structural_fields: Default::default(),
            structural_cases: Vec::new(),
            element_views: std::collections::BTreeMap::new(),
            view_locals: Vec::new(),
            byte_record_loans: std::sync::Arc::from([]),
        }
    }

    pub(crate) fn new(parameters: impl IntoIterator<Item = ScalarType>) -> Self {
        Self::for_computation_operands(0, parameters)
    }

    pub(crate) fn with_structural_parameters(
        mut self,
        parameters: &[(u32, StructuralParameterDeclaration)],
    ) -> Self {
        self.structural_parameters = parameters.to_vec();
        self.structural_fields.clear_parameters();
        self.structural_cases.clear();
        // Element views are keyed by structural type, not by parameter: a
        // type's view family stays true under a rebound parameter list, so a
        // namespace rebinding its parameters keeps the element views its
        // observations already resolved.
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

    /// Retain the view locals the ordinary operation sequence established, so
    /// a length or element read rooted at one observes its published place.
    pub(crate) fn with_view_locals(mut self, locals: &[view_locals::ViewLocalBinding]) -> Self {
        self.view_locals = locals.to_vec();
        self
    }

    pub(crate) fn with_byte_record_loans(
        mut self,
        loans: &std::sync::Arc<
            [crate::expression_preparation::bindings::recast_records::ByteRecordLoan],
        >,
    ) -> Self {
        self.byte_record_loans = loans.clone();
        self
    }

    /// Resolve the backing at the observation, without establishing a second
    /// place for a recast marker or granting a read through write-only custody.
    pub(crate) fn byte_record_backing(
        &self,
        source: crate::expression_preparation::bindings::recast_records::LoanedSource,
    ) -> Result<PlaceId, LoweringError> {
        use crate::expression_preparation::bindings::recast_records::LoanedSource;
        match source {
            LoanedSource::Parameter(position) => {
                let (_, parameter) = self.structural_parameters.get(position as usize).ok_or(
                    LoweringError::Unsupported(
                        "record byte observation lost its backing parameter",
                    ),
                )?;
                if !matches!(
                    parameter.access,
                    StructuralAccess::Owned
                        | StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                ) || !parameter.projected_qualifications.is_empty()
                {
                    return unsupported("record byte observation changed its parameter custody");
                }
                Ok(parameter.place)
            }
            LoanedSource::Local(symbol) => {
                let mut candidates = self
                    .structural_locals
                    .iter()
                    .filter(|local| local.symbol == symbol);
                let Some(StructuralLocalBinding { argument, .. }) = candidates.next() else {
                    return unsupported("record byte observation lost its live local backing");
                };
                if candidates.next().is_some()
                    || !symbol.is_valid()
                    || !argument.path.is_empty()
                    || !matches!(
                        argument.access,
                        StructuralAccess::Owned
                            | StructuralAccess::SharedBorrow
                            | StructuralAccess::MutableBorrow
                    )
                {
                    return unsupported("record byte observation changed its local custody");
                }
                Ok(argument.place)
            }
        }
    }

    pub(crate) fn byte_record_observation(
        &self,
        symbol: symbols::SymbolHandle,
        path: &[checked_trees::CheckedStructuralPredicatePathSegment],
        index: &CheckedScalarExpression,
        inner_indexes: &[CheckedScalarExpression],
        element_path: &[checked_trees::CheckedStructuralPredicatePathSegment],
        primitive: PrimitiveType,
    ) -> Result<Option<LoweredDirectExpression>, LoweringError> {
        crate::expression_preparation::bindings::recast_records::byte_record_observation(
            &self.byte_record_loans,
            self,
            symbol,
            path,
            index,
            inner_indexes,
            element_path,
            primitive,
        )
    }

    /// Resolve an independently replayed borrowed view to the activation's
    /// live descriptor. A range never substitutes an equal-typed backing.
    pub(crate) fn borrowed_view_source(
        &self,
        root: checked_trees::CheckedStorageRoot,
        parameters: &[checked_trees::signature::StateParameter],
        checked: &CheckedTrees,
        structural_type: StructuralTypeId,
        bytes: bool,
        access: StructuralAccess,
    ) -> Result<(symbols::SymbolHandle, PlaceId), LoweringError> {
        match root {
            checked_trees::CheckedStorageRoot::Parameter { index } => {
                let (position, parameter) = parameters
                    .iter()
                    .enumerate()
                    .filter(|(_, parameter)| {
                        !parameter.relevance.is_erased()
                            && checked
                                .primitive_type_reference(parameter.type_reference)
                                .is_none()
                    })
                    .nth(index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "computed view source parameter is absent",
                    ))?;
                let mut matching = self
                    .structural_parameters
                    .iter()
                    .filter(|(authored, _)| *authored as usize == position);
                let (_, source) = matching.next().ok_or(LoweringError::Unsupported(
                    "computed view source has no live descriptor",
                ))?;
                if matching.next().is_some()
                    || source.structural_type != structural_type
                    || !matches!(
                        access,
                        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                    )
                    || !(source.access == access
                        || source.access == StructuralAccess::MutableBorrow
                            && access == StructuralAccess::SharedBorrow)
                    || source.multiplicity != StructuralMultiplicity::Unrestricted
                    || (!bytes && !source.qualifications.is_empty())
                    || !source.projected_qualifications.is_empty()
                    || bytes == self.element_views.contains_key(&structural_type)
                {
                    return unsupported("computed view source changed type or borrowed custody");
                }
                Ok((parameter.symbol, source.place))
            }
            checked_trees::CheckedStorageRoot::ViewLocal { symbol } => {
                let local = view_locals::resolve(&self.view_locals, symbol)?;
                if local.structural_type != structural_type
                    || !matches!(
                        access,
                        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                    )
                    || !(local.access == access
                        || local.access == StructuralAccess::MutableBorrow
                            && access == StructuralAccess::SharedBorrow)
                    || bytes != (local.carrier == view_locals::ViewCarrier::Bytes)
                {
                    return unsupported("computed view local changed type or family");
                }
                Ok((symbol, local.place))
            }
            checked_trees::CheckedStorageRoot::OwnedLocal { .. } => {
                unsupported("computed view source cannot be owned storage")
            }
        }
    }

    /// Retain places established by the ordinary structural operation sequence.
    pub(crate) fn with_structural_locals(mut self, locals: &[StructuralLocalBinding]) -> Self {
        self.structural_locals = locals.to_vec();
        self
    }

    /// Resolve the exact primitive endpoint of an already source-validated loan.
    /// A local reference owns a carrier, not a new primitive home: its retained
    /// `Referent` projection reaches the viewed type while the reference keeps
    /// the original backing identity. The same catalog walk checks parameter
    /// and local projections, including authority at every reference crossing.
    pub(crate) fn primitive_borrow(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        scalar_type: ScalarType,
        structural_types: &[StructuralTypeDeclaration],
    ) -> Result<StructuralArgument, LoweringError> {
        use structural_paths::PreparedArgumentTip;
        use terminal_psi::{StructuralFieldType, StructuralTypeShape};
        let access = structural_paths::terminal_access(argument.access);
        let permits = |granted, requested| {
            matches!(
                (granted, requested),
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
        };
        if access == StructuralAccess::Owned {
            return unsupported("computed primitive argument cannot transfer ownership");
        }
        let (place, root_type) = match argument.source {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } => {
                if !argument.path.is_empty() {
                    return unsupported(
                        "computed primitive storage argument is not a whole referent",
                    );
                }
                return Ok(StructuralArgument {
                    place: primitive_storage_place(&self.primitive_storage, symbol, scalar_type)?,
                    path: Vec::new(),
                    access,
                });
            }
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                let mut locals = self
                    .structural_locals
                    .iter()
                    .filter(|row| row.symbol == symbol);
                let source = locals.next().ok_or(LoweringError::Unsupported(
                    "computed primitive borrow lost its established local",
                ))?;
                if !symbol.is_valid()
                    || locals.next().is_some()
                    || source.argument.access != StructuralAccess::SharedBorrow
                    || !source.argument.path.is_empty()
                    || access != StructuralAccess::SharedBorrow
                {
                    return unsupported("computed primitive borrow changes its local custody");
                }
                (source.argument.place, source.structural_type)
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
                    || !permits(parameter.access, access)
                {
                    return unsupported("computed primitive borrow widens parameter custody");
                }
                (parameter.place, parameter.structural_type)
            }
            _ => return unsupported("computed primitive borrow has no supported source place"),
        };
        let projection = structural_paths::prepare_argument_projection(
            &argument.path,
            root_type,
            structural_types,
        )?;
        let actual_type = match projection.tip {
            PreparedArgumentTip::Declaration(declaration) => match declaration.shape {
                StructuralTypeShape::PrimitiveScalar(primitive) => Some(primitive),
                _ => None,
            },
            PreparedArgumentTip::Inline(StructuralFieldType::Scalar(primitive)) => Some(*primitive),
            _ => None,
        };
        if actual_type != Some(scalar_type)
            || projection
                .crossed_accesses
                .iter()
                .any(|granted| !permits(*granted, access))
        {
            return unsupported(
                "computed primitive borrow changed its endpoint or reference authority",
            );
        }
        Ok(StructuralArgument {
            place,
            path: structural_paths::lower_structural_path(&argument.path)?,
            access,
        })
    }

    /// The owned home a whole copyable local's snapshot reads. The copy plan
    /// is a shared read of the complete local; binding this home into a fresh
    /// unrestricted home copies its current contents and leaves it live.
    pub(crate) fn copied_local_home(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    ) -> Result<StructuralArgument, LoweringError> {
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } =
            argument.source
        else {
            return unsupported("copied record requires an established local");
        };
        let mut locals = self
            .structural_locals
            .iter()
            .filter(|row| row.symbol == symbol);
        let StructuralLocalBinding {
            argument: source, ..
        } = locals.next().ok_or(LoweringError::Unsupported(
            "copied record lost its established local",
        ))?;
        if !symbol.is_valid()
            || locals.next().is_some()
            || !argument.path.is_empty()
            || argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow
            || !source.path.is_empty()
            || source.access != StructuralAccess::Owned
        {
            return unsupported("copied record changes its source custody");
        }
        Ok(source.clone())
    }

    /// Resolve a source-validated whole owned parameter without inventing a loan.
    pub(crate) fn owned_argument(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    ) -> Result<StructuralArgument, LoweringError> {
        if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } =
            argument.source
        {
            let mut locals = self
                .structural_locals
                .iter()
                .filter(|row| row.symbol == symbol);
            let StructuralLocalBinding {
                argument: source, ..
            } = locals.next().ok_or(LoweringError::Unsupported(
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
        self.authored[position] = None;
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
            CheckedScalarBindingDestination::Immutable => {
                self.immutable.push(Some(position));
                self.authored.push(Some((position, scalar_type)));
            }
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

    /// Bind a retained scalar result that no source name reads: a call result
    /// an assignment stores. It takes its binding-ordinal slot, but authored
    /// `Local` reads keep counting only declared locals.
    pub(crate) fn append_unnamed_result(&mut self, position: usize) {
        self.immutable.push(Some(position));
    }

    /// The dense value position of one authored `Parameter`/`Local` slot:
    /// the scalar formals, then each declared immutable local in order.
    fn authored_position(&self, position: usize) -> Result<usize, LoweringError> {
        self.authored
            .get(position)
            .copied()
            .flatten()
            .map(|(position, _)| position)
            .ok_or(LoweringError::Unsupported(
                "scalar immutable operand is outside the established namespace",
            ))
    }

    /// The dense value position of one binding-ordinal slot: the scalar
    /// formals, then each established scalar result in binding order.
    pub(crate) fn immutable_position(&self, position: usize) -> Result<usize, LoweringError> {
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
            CheckedScalarExpression::StructuralParameterIndexedRead {
                index,
                inner_indexes,
                ..
            } => {
                self.scalar(index)?;
                for selector in inner_indexes {
                    self.scalar(selector)?;
                }
            }
            CheckedScalarExpression::Parameter { position, .. }
            | CheckedScalarExpression::Local { position, .. } => {
                *position = self.authored_position(*position)?
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
            | CheckedScalarExpression::RepresentationView { operand, .. }
            | CheckedScalarExpression::IntegerSaturatingCast { operand, .. }
            | CheckedScalarExpression::IntegerExactCast { operand, .. }
            | CheckedScalarExpression::IntegerTrappingCast { operand, .. } => {
                self.scalar(operand)?
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
            CheckedBooleanExpression::IntegerMathComparison { .. } => {
                return unsupported("mathematical contract predicate has no executable binding");
            }
            CheckedBooleanExpression::Parameter { position }
            | CheckedBooleanExpression::Local { position } => {
                *position = self.authored_position(*position)?
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
            CheckedBooleanExpression::ByteSequencePredicate { path, .. } => {
                for segment in path.iter_mut() {
                    let checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(index) =
                        segment
                    else {
                        continue;
                    };
                    match index {
                        checked_trees::CheckedRuntimeIndex::Parameter { position }
                        | checked_trees::CheckedRuntimeIndex::Local { position } => {
                            *position = u32::try_from(self.authored_position(*position as usize)?)
                                .map_err(|_| {
                                    LoweringError::Unsupported(
                                        "byte predicate selector overflows its position carrier",
                                    )
                                })?;
                        }
                        _ => {
                            return unsupported("byte predicate cannot rebind a captured selector");
                        }
                    }
                }
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
            .ok_or(LoweringError::Unsupported(
                "scalar computation needs one checked expression and one source binding",
            ))?;
        let expression = self.expression(expression)?;
        source_custody::validate_pure(checked, binding, expression.scalar_type())?;
        Ok(expression)
    }

    /// Observe a checked index field at its authored operand position. The call
    /// expansion carries this value across later argument effects.
    pub(crate) fn runtime_index_field(
        &self,
        field: &checked_trees::CheckedStructuralParameterField,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let (source, path, field, scalar_type) = structural_fields::resolve_typed(
            &self.structural_fields,
            field.parameter_position,
            &field.path,
        )?;
        if !matches!(scalar_type, ScalarType::Integer(integer) if !integer.is_address()) {
            return unsupported("runtime index field selector is not an integer read");
        }
        Ok(LoweredDirectExpression::StructuralField {
            source,
            path,
            field,
            scalar_type,
        })
    }

    /// Resolve an authored scalar coordinate before emission. Runtime path
    /// coordinates have not undergone the ordinary expression remapping.
    pub(crate) fn runtime_index_parameter(
        &self,
        position: u32,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        if position as usize >= self.parameter_count {
            return unsupported("runtime selector parameter names a non-parameter scalar");
        }
        self.runtime_index_value(position)
    }

    pub(crate) fn runtime_index_local(
        &self,
        position: u32,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        if (position as usize) < self.parameter_count {
            return unsupported("runtime selector local names a parameter scalar");
        }
        self.runtime_index_value(position)
    }

    fn runtime_index_value(&self, position: u32) -> Result<LoweredDirectExpression, LoweringError> {
        let (position, scalar_type) = self
            .authored
            .get(position as usize)
            .copied()
            .flatten()
            .ok_or(LoweringError::Unsupported(
                "runtime selector has no established immutable scalar",
            ))?;
        if !matches!(scalar_type, ScalarType::Integer(integer) if !integer.is_address()) {
            return unsupported("runtime selector is not an integer");
        }
        Ok(LoweredDirectExpression::Parameter {
            position,
            scalar_type,
        })
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
            &self.view_locals,
            &self.structural_locals,
            Some(self),
        )
    }
}

/// The element views among `parameters`' types, with the scalar each element
/// read yields (absent for record elements, which have a length but no scalar
/// read). Byte views stay on the byte path and never appear here.
pub(crate) fn element_views(
    parameters: &[(u32, StructuralParameterDeclaration)],
    types: &[StructuralTypeDeclaration],
) -> std::collections::BTreeMap<StructuralTypeId, Option<ScalarType>> {
    parameters
        .iter()
        .filter_map(|(_, parameter)| {
            let shape = types
                .iter()
                .find(|declaration| declaration.id == parameter.structural_type)?;
            let terminal_psi::StructuralTypeShape::ElementView { element } = shape.shape else {
                return None;
            };
            let element = types.iter().find(|declaration| declaration.id == element)?;
            let scalar_type = match element.shape {
                terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                    Some(scalar_type)
                }
                _ => None,
            };
            Some((parameter.structural_type, scalar_type))
        })
        .collect()
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
