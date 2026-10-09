//! Resolve authored scalar names against their evaluated values, and checked
//! storage reads against their primitive places.
//! Once an owned value acquires an address, its authored binding observes that
//! original home. A separate copied binding still names its evaluated SSA value.
//! Binding replacement permission is not permission to mutate borrowed contents.
use super::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees,
    LoweringError, PlaceId, PrimitiveType, ScalarType, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeId, terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use terminal_psi::find_structural_type;

pub(crate) mod primitive_reads;
pub(crate) use primitive_reads::primitive_storage_read;
pub(crate) mod place_observations;
pub(crate) mod structural_cases;
mod structural_copy;
pub(crate) mod structural_fields;
pub(crate) mod structural_paths;
pub(crate) use structural_copy::{PreparedCopyPathSegment, PreparedStructuralCopy};
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
    /// The access the Terminal structural argument carries for this loan.
    pub(crate) fn structural_access(self) -> StructuralAccess {
        match self {
            Self::Shared => StructuralAccess::SharedBorrow,
            Self::Mutable => StructuralAccess::MutableBorrow,
        }
    }
}

/// The exact published carrier stays with its source identity and custody.
/// A reference result may retain its reference shell even when its argument
/// access is shared; access alone therefore cannot recover this type.
#[derive(Clone, Debug)]
pub(crate) struct StructuralLocalBinding {
    /// The exact published result namespace alias, absent for parameter or
    /// constructor homes established without an operation result binding.
    pub(crate) result_binding_ordinal: Option<u32>,
    pub(crate) root: facts::PlaceRoot,
    pub(crate) argument: StructuralArgument,
    pub(crate) structural_type: StructuralTypeId,
}

/// The structural local binding rooted at `root`.
pub(crate) fn structural_local_by_root(
    locals: &[StructuralLocalBinding],
    root: facts::PlaceRoot,
) -> Option<&StructuralLocalBinding> {
    locals.iter().find(|local| local.root == root)
}

fn valid_structural_root(root: facts::PlaceRoot) -> bool {
    match root {
        facts::PlaceRoot::Symbol(symbol) => symbol.is_valid(),
        facts::PlaceRoot::Expression(expression) => expression.is_valid(),
        _ => false,
    }
}

/// Resolve the existing owned home, retaining any projected alias prefix.
/// Loan completion and computed-call preparation use the same binding join.
pub(crate) fn owned_structural_home(
    root: facts::PlaceRoot,
    source_parameters: &[checked_trees::signature::StateParameter],
    parameters: &[(u32, StructuralParameterDeclaration)],
    locals: &[StructuralLocalBinding],
) -> Result<(StructuralArgument, StructuralTypeId), LoweringError> {
    let mut bindings = locals.iter().filter(|binding| binding.root == root);
    let home = if let Some(binding) = bindings.next() {
        if bindings.next().is_some() {
            return unsupported("owned storage has duplicate source bindings");
        }
        (binding.argument.clone(), binding.structural_type)
    } else {
        let position = source_parameters
            .iter()
            .position(|parameter| Some(parameter.symbol) == root.as_symbol())
            .ok_or(LoweringError::Unsupported(
                "owned storage has no source binding",
            ))?;
        let (_, parameter) = parameters
            .iter()
            .find(|(source, _)| *source as usize == position)
            .ok_or(LoweringError::Unsupported(
                "owned storage lost its structural parameter",
            ))?;
        (
            StructuralArgument {
                place: parameter.place,
                path: Vec::new(),
                access: parameter.access,
            },
            parameter.structural_type,
        )
    };
    if home.0.access != StructuralAccess::Owned {
        return unsupported("loan completion cannot establish borrowed ingress custody");
    }
    Ok(home)
}

fn published_structural_type(
    parameters: &[(u32, StructuralParameterDeclaration)],
    locals: &[StructuralLocalBinding],
    place: PlaceId,
) -> Option<StructuralTypeId> {
    let mut types = parameters
        .iter()
        .filter(|(_, source)| source.place == place)
        .map(|(_, source)| source.structural_type)
        .chain(
            locals
                .iter()
                .filter(|local| local.argument.place == place && local.argument.path.is_empty())
                .map(|local| local.structural_type),
        );
    let structural_type = types.next()?;
    types
        .all(|candidate| candidate == structural_type)
        .then_some(structural_type)
}

/// One authored binding's evaluated value and, once addressed, its storage
/// identity. The zero symbol keeps unaddressed bindings as SSA values.
#[derive(Clone, Copy)]
struct AuthoredScalarBinding {
    value_position: usize,
    scalar_type: ScalarType,
    storage_symbol: symbols::SymbolHandle,
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
    authored: Vec<Option<AuthoredScalarBinding>>,
    parameter_count: usize,
    primitive_storage: Vec<(symbols::SymbolHandle, PlaceId, ScalarType)>,
    /// Actual original ingress homes, in the dense scalar formal namespace.
    scalar_parameter_homes: Vec<(u32, PlaceId, ScalarType, StructuralAccess)>,
    /// Authored parameter positions stay separate from dense Terminal positions.
    structural_parameters: Vec<(u32, StructuralParameterDeclaration)>,
    structural_locals: Vec<StructuralLocalBinding>,
    structural_fields: StructuralFieldBindings,
    structural_cases: Vec<structural_cases::StructuralCaseBinding>,
    /// Whole element-view parameters resolved to their scalar element type,
    /// absent for record elements (which have a length but no scalar read).
    /// `&[u8]` stays on the byte path.
    element_views: std::collections::BTreeMap<StructuralTypeId, Option<ScalarType>>,
    /// Established immutable view locals an observation may root at.
    view_locals: Vec<view_locals::ViewLocalBinding>,
    place_observations: std::sync::Arc<
        [crate::expression_preparation::bindings::place_observations::PlaceObservation],
    >,
}

impl ScalarBindings {
    pub(crate) fn owned_structural_home(
        &self,
        root: facts::PlaceRoot,
        source_parameters: &[checked_trees::signature::StateParameter],
    ) -> Result<(StructuralArgument, StructuralTypeId), LoweringError> {
        owned_structural_home(
            root,
            source_parameters,
            &self.structural_parameters,
            &self.structural_locals,
        )
    }
    /// The recorded scalar-ingress home of dense formal `index`, when present.
    fn scalar_parameter_home_row(
        &self,
        index: u32,
    ) -> Option<&(u32, PlaceId, ScalarType, StructuralAccess)> {
        self.scalar_parameter_homes
            .iter()
            .find(|row| row.0 == index)
    }

    /// Resolve the source home before lowering the argument's relative path.
    /// A projected alias retains its physical root and its established prefix.
    pub(crate) fn borrowed_structural_home(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        _machine: &checked_trees::machine::Machine,
        parameters: &[checked_trees::signature::StateParameter],
    ) -> Result<StructuralArgument, LoweringError> {
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
        // preservation contract for both root and descendant memberships.
        let supports_parameter = |source: &StructuralParameterDeclaration| {
            supports_borrow(source.access)
                && ((source.qualifications.is_empty()
                    && source.projected_qualifications.is_empty())
                    || argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow)
        };
        let storage_root = argument
            .structural_storage_root()
            .or_else(|| match argument.source {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                    root: checked_trees::CheckedStorageRoot::OwnedLocal { symbol },
                    ..
                }
                | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                    root: checked_trees::CheckedStorageRoot::OwnedLocal { symbol },
                    ..
                } => Some(facts::PlaceRoot::Symbol(symbol)),
                _ => None,
            });
        let place = match argument.source {
            _ if storage_root.is_some() => {
                let root = storage_root.ok_or(LoweringError::Unsupported(
                    "computed borrow lost its storage identity",
                ))?;
                let mut locals = self.structural_locals.iter().filter(|row| row.root == root);
                match locals.next() {
                    Some(source) => {
                        // An owned local lends its whole place; a `&T` local
                        // is itself a shared-borrow join result, so borrowing
                        // through it reuses that same shared custody rather
                        // than fabricating ownership.
                        if !valid_structural_root(root)
                            || locals.next().is_some()
                            || !supports_borrow(source.argument.access)
                            || self.structural_argument_type(&source.argument)
                                != Some(source.structural_type)
                        {
                            return unsupported("computed borrow changes its local custody");
                        }
                        source.argument.clone()
                    }
                    None => {
                        let Some(symbol) = root.as_symbol() else {
                            return unsupported("computed borrow lost its established expression");
                        };
                        let position =
                            typed_trees::signature::state_parameter_position(parameters, symbol)
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
                        StructuralArgument {
                            place: source.place,
                            path: Vec::new(),
                            access: source.access,
                        }
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
                StructuralArgument {
                    place: source.place,
                    path: Vec::new(),
                    access: source.access,
                }
            }
            _ => return unsupported("computed borrow has no established source"),
        };
        Ok(StructuralArgument {
            access: structural_paths::terminal_access(argument.access),
            ..place
        })
    }

    /// Join a source observation's declared root to the storage actually
    /// published for it. Borrowed ingress already names its referent; a stored
    /// reference instead requires an explicit pointer crossing.
    pub(crate) fn structural_observation_path(
        &self,
        source: &StructuralArgument,
        observed_root: StructuralTypeId,
        path: &[terminal_psi::StructuralPathSegment],
    ) -> Result<Vec<terminal_psi::StructuralPathSegment>, LoweringError> {
        let root = self
            .structural_argument_type(source)
            .ok_or(LoweringError::Unsupported(
                "structural observation has no established root type",
            ))?;
        let mut selected = source.path.clone();
        if root == observed_root {
            selected.extend_from_slice(path);
            return Ok(selected);
        }
        let declaration = self
            .structural_fields
            .declarations()
            .iter()
            .find(|row| row.id == root);
        if !matches!(declaration.map(|row| &row.shape),
            Some(terminal_psi::StructuralTypeShape::Reference { referent, access })
                if *referent == observed_root && matches!(access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow))
        {
            return unsupported("structural observation changes its established root type");
        }
        selected.push(terminal_psi::StructuralPathSegment::Referent);
        selected.extend_from_slice(path);
        Ok(selected)
    }

    pub(crate) fn structural_declarations(&self) -> &[StructuralTypeDeclaration] {
        self.structural_fields.declarations()
    }

    /// Resolve the exact published carrier at a live source place.
    pub(crate) fn structural_place_type(&self, place: PlaceId) -> Option<StructuralTypeId> {
        published_structural_type(&self.structural_parameters, &self.structural_locals, place)
    }

    /// The selected alias type is distinct from the physical root declaration.
    pub(crate) fn structural_argument_type(
        &self,
        source: &StructuralArgument,
    ) -> Option<StructuralTypeId> {
        let root = self.structural_place_type(source.place)?;
        if source.path.is_empty() {
            return Some(root);
        }
        terminal_semantics::runtime_structural_path_tip(
            self.structural_fields.declarations().iter(),
            root,
            &source.path,
        )
        .map(|declaration| declaration.id)
    }

    /// Borrow the established referent, not a copy of its scalar field values.
    /// Source replay checks the declaration and loan occurrence; this join binds
    /// that source to the current activation's exact local or parameter place.
    /// `parameters` is the authored signature list: `StructuralLocal` names a
    /// live local or an authored parameter directly by symbol.
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
        let mut source = self.borrowed_structural_home(argument, machine, parameters)?;
        let root = self
            .structural_argument_type(&source)
            .ok_or(LoweringError::Unsupported(
                "borrowed argument lost its published selected type",
            ))?;
        let prepared = structural_paths::prepare_structural_argument_path(
            argument,
            root,
            self.structural_declarations(),
        )?;
        source.path.extend(structural_paths::emit_argument_path(
            &prepared,
            None,
            &mut std::iter::empty(),
            None,
        )?);
        source.access = access;
        Ok(source)
    }

    /// Observe a runtime-selected element through its resolved source home.
    /// The checked path
    /// carries exactly one `RuntimeIndex`; its selector is read first
    /// and becomes the observation's index, and the static halves on either
    /// side resolve against the selected type's declarations. Static paths use
    /// the same declaration walk without evaluating a selector.
    pub(crate) fn element_case_observation(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        source: &StructuralArgument,
        case: &str,
    ) -> Result<LoweredBooleanReturnExpression, LoweringError> {
        let structural_type =
            self.structural_argument_type(source)
                .ok_or(LoweringError::Unsupported(
                    "case observation lost its established source type",
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
        let (place, path, element_path, case, observes_element_view_length) =
            structural_cases::resolve_runtime_element(
                self.structural_fields.declarations(),
                structural_type,
                source.place,
                &static_path(&argument.path[..selector_position])?,
                &static_path(&argument.path[selector_position + 1..])?,
                case,
            )?;
        let index = match selector {
            checked_trees::CheckedRuntimeIndex::Parameter { position } => {
                self.runtime_index_parameter(*position)?
            }
            checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
                structural_fields::resolve_typed(
                    &self.structural_fields,
                    field.parameter_position,
                    &field.path,
                )?
            }
            // Hoisted computations and addressed primitive locals share the
            // authored value/home join with ordinary element observations.
            checked_trees::CheckedRuntimeIndex::Local { position } => {
                self.runtime_index_local(*position)?
            }
            _ => return unsupported("runtime case observation requires a bound scalar selector"),
        };
        if !matches!(
            index.scalar_type(),
            ScalarType::Integer(integer) if !integer.is_address()
        ) {
            return unsupported("runtime case observation selector is not an integer read");
        }
        if observes_element_view_length && !source.path.is_empty() {
            return unsupported("nested element view requires its established descriptor home");
        }
        let mut selected = source.path.clone();
        selected.extend(path);
        Ok(LoweredBooleanReturnExpression::IndexedCaseMembership {
            source: place,
            observes_element_view_length,
            path: selected,
            index: Box::new(index),
            element_path,
            case,
        })
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
            .map(|(position, scalar_type)| {
                Some(AuthoredScalarBinding {
                    value_position: offset + position,
                    scalar_type,
                    storage_symbol: symbols::SymbolHandle::invalid(),
                })
            })
            .collect::<Vec<_>>();
        Self {
            immutable: authored
                .iter()
                .map(|binding| binding.map(|row| row.value_position))
                .collect(),
            parameter_count: authored.len(),
            authored,
            primitive_storage: Vec::new(),
            scalar_parameter_homes: Vec::new(),
            structural_parameters: Vec::new(),
            structural_locals: Vec::new(),
            structural_fields: Default::default(),
            structural_cases: Vec::new(),
            element_views: std::collections::BTreeMap::new(),
            view_locals: Vec::new(),
            place_observations: std::sync::Arc::from([]),
        }
    }

    pub(crate) fn new(parameters: impl IntoIterator<Item = ScalarType>) -> Self {
        Self::for_computation_operands(0, parameters)
    }

    /// A body's entry namespace: its scalar formals over the storage and
    /// observation layers every authored read resolves against. Later scalar
    /// results append to it, so an unnamed call result or a private slot never
    /// shifts an authored local onto another value.
    pub(crate) fn entry(
        formals: impl IntoIterator<Item = ScalarType>,
        primitive_storage: &[(symbols::SymbolHandle, PlaceId, ScalarType)],
        parameters: &[(u32, StructuralParameterDeclaration)],
        structural_types: &[StructuralTypeDeclaration],
        place_observations: &std::sync::Arc<
            [crate::expression_preparation::bindings::place_observations::PlaceObservation],
        >,
    ) -> Self {
        Self::new(formals)
            .with_primitive_storage(primitive_storage)
            .with_structural_parameters(parameters)
            .with_structural_observations(structural_types)
            .with_place_observations(place_observations)
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
    pub(crate) fn with_primitive_storage(
        mut self,
        storage: &[(symbols::SymbolHandle, PlaceId, ScalarType)],
    ) -> Self {
        self.primitive_storage = storage.to_vec();
        self
    }

    pub(crate) fn register_scalar_parameter_home(
        &mut self,
        parameter_index: u32,
        place: PlaceId,
        scalar_type: ScalarType,
        access: StructuralAccess,
    ) -> Result<(), LoweringError> {
        if let Some((_, original, original_type, original_access)) =
            self.scalar_parameter_home_row(parameter_index)
        {
            if *original != place || *original_type != scalar_type || *original_access != access {
                return unsupported("scalar ingress changes its original home");
            }
        } else {
            self.scalar_parameter_homes
                .push((parameter_index, place, scalar_type, access));
        }
        Ok(())
    }

    pub(crate) fn scalar_parameter_home(
        &self,
        parameter_index: u32,
        scalar_type: ScalarType,
    ) -> Result<Option<PlaceId>, LoweringError> {
        match self.scalar_parameter_home_row(parameter_index) {
            Some((_, place, actual, _)) if *actual == scalar_type => Ok(Some(*place)),
            Some(_) => unsupported("scalar ingress home changes its carrier"),
            None => Ok(None),
        }
    }

    /// Retain the view locals the ordinary operation sequence established, so
    /// a length or element read rooted at one observes its published place.
    pub(crate) fn with_view_locals(mut self, locals: &[view_locals::ViewLocalBinding]) -> Self {
        self.view_locals = locals.to_vec();
        self
    }

    pub(crate) fn with_place_observations(
        mut self,
        loans: &std::sync::Arc<
            [crate::expression_preparation::bindings::place_observations::PlaceObservation],
        >,
    ) -> Self {
        self.place_observations = loans.clone();
        self
    }

    /// Captured coordinates are private scalar homes, never authored local slots.
    pub(crate) fn observation_coordinate(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let mut matching = self
            .place_observations
            .iter()
            .filter(|loan| loan.symbol == symbol);
        let loan = matching.next().ok_or(LoweringError::Unsupported(
            "record loan has no captured coordinate",
        ))?;
        if !symbol.is_valid() || matching.next().is_some() {
            return unsupported("record loan coordinate has ambiguous identity");
        }
        let capture = loan.capture.as_ref().map_err(Clone::clone)?;
        let source = capture.coordinate.ok_or(LoweringError::Unsupported(
            "record loan coordinate was not captured at formation",
        ))?;
        Ok(LoweredDirectExpression::PrimitiveRead {
            source: crate::emission::operation_emission::expressions::PrimitiveReadSource::Place(
                source,
            ),
            path: Vec::new(),
            scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
        })
    }

    /// Resolve the backing at the observation, without establishing a second
    /// place for a recast marker or granting a read through write-only custody.
    pub(crate) fn observation_backing(
        &self,
        source: crate::expression_preparation::bindings::place_observations::LoanedSource,
    ) -> Result<PlaceId, LoweringError> {
        use crate::expression_preparation::bindings::place_observations::LoanedSource;
        match source {
            LoanedSource::Parameter(position) => {
                let (_, parameter) = self.structural_parameters.get(position as usize).ok_or(
                    LoweringError::Unsupported(
                        "record byte observation lost its backing parameter",
                    ),
                )?;
                Ok(parameter.place)
            }
            LoanedSource::Local(symbol) => {
                // A loan names its original storage, regardless of whether the
                // owner is a primitive local or a structural local. Recasting
                // changes the observation type, not that storage identity.
                let mut homes = self
                    .primitive_storage
                    .iter()
                    .filter(|(candidate, _, _)| *candidate == symbol)
                    .map(|(_, place, _)| *place)
                    .chain(
                        self.structural_locals
                            .iter()
                            .filter(|local| local.root == facts::PlaceRoot::Symbol(symbol))
                            .map(|local| local.argument.place),
                    );
                let place = homes.next().ok_or(LoweringError::Unsupported(
                    "place observation lost its live local backing",
                ))?;
                if !symbol.is_valid() || homes.next().is_some() {
                    return unsupported("place observation has ambiguous local backing");
                }
                Ok(place)
            }
        }
    }

    pub(crate) fn byte_record_observation(
        &self,
        proven_indexes: &[checked_trees::expression::ExpressionHandle],
        symbol: symbols::SymbolHandle,
        path: &[checked_trees::CheckedStructuralPredicatePathSegment],
        index: &CheckedScalarExpression,
        element_path: &[checked_trees::CheckedScalarReadProjection],
        primitive: PrimitiveType,
    ) -> Result<Option<LoweredDirectExpression>, LoweringError> {
        crate::expression_preparation::bindings::place_observations::byte_record_observation(
            &self.place_observations,
            self,
            proven_indexes,
            symbol,
            path,
            index,
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
    /// Root ownership supplies a temporary loan; it is not the argument access.
    /// Keep that owner's home and cleanup, then attenuate at every descriptor
    /// crossing so owning a record cannot widen a shared reference inside it.
    pub(crate) fn primitive_borrow(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        scalar_type: ScalarType,
        structural_types: &[StructuralTypeDeclaration],
    ) -> Result<StructuralArgument, LoweringError> {
        let source = self.prepared_primitive_borrow(argument, scalar_type, structural_types)?;
        Ok(StructuralArgument {
            place: source.place,
            access: source.access,
            path: structural_paths::emit_argument_path(
                &source.path,
                None,
                &mut std::iter::empty(),
                None,
            )?,
        })
    }

    /// A borrow of a scalar formal lends the home its address demand
    /// published, or a `mut` formal's storage, through the same join a
    /// computed call operand takes.
    pub(crate) fn scalar_parameter_borrow(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        structural_types: &[StructuralTypeDeclaration],
    ) -> Result<StructuralArgument, LoweringError> {
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ScalarParameter {
            parameter_index,
        } = argument.source
        else {
            return unsupported("scalar formal borrow names another source");
        };
        let (_, _, scalar_type, _) =
            self.scalar_parameter_home_row(parameter_index)
                .ok_or(LoweringError::Unsupported(
                    "borrowed scalar formal has no published home",
                ))?;
        self.primitive_borrow(argument, *scalar_type, structural_types)
    }

    pub(crate) fn prepared_primitive_borrow(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        scalar_type: ScalarType,
        structural_types: &[StructuralTypeDeclaration],
    ) -> Result<crate::emission::operation_emission::calls::PreparedStructuralArgument, LoweringError>
    {
        let access = structural_paths::terminal_access(argument.access);
        let permits = |granted, requested| {
            matches!(
                (granted, requested),
                (
                    StructuralAccess::SharedBorrow,
                    StructuralAccess::SharedBorrow
                ) | (
                    StructuralAccess::Owned | StructuralAccess::MutableBorrow,
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
        let (source, root_type) = match argument.source {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::ScalarParameter {
                parameter_index,
            } => {
                if !argument.path.is_empty() {
                    return unsupported(
                        "scalar ingress borrow projects outside its primitive home",
                    );
                }
                let place = self
                    .scalar_parameter_home(parameter_index, scalar_type)?
                    .ok_or(LoweringError::Unsupported(
                        "computed primitive ingress has no established original home",
                    ))?;
                let (_, _, _, granted) = self.scalar_parameter_home_row(parameter_index).ok_or(
                    LoweringError::Unsupported("computed ingress lost its access custody"),
                )?;
                if !permits(*granted, access) {
                    return unsupported("computed primitive ingress widens original access");
                }
                return crate::emission::operation_emission::calls::PreparedStructuralArgument::from_static(StructuralArgument { place, path: Vec::new(), access });
            }
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } => {
                if !argument.path.is_empty() {
                    return unsupported(
                        "computed primitive storage argument is not a whole referent",
                    );
                }
                return crate::emission::operation_emission::calls::PreparedStructuralArgument::from_static(StructuralArgument {
                    place: primitive_storage_place(&self.primitive_storage, symbol, scalar_type)?,
                    path: Vec::new(),
                    access,
                });
            }
            _ if argument.structural_storage_root().is_some() => {
                let root = argument
                    .structural_storage_root()
                    .ok_or(LoweringError::Unsupported(
                        "computed primitive borrow lost its storage identity",
                    ))?;
                let mut locals = self.structural_locals.iter().filter(|row| row.root == root);
                let source = locals.next().ok_or(LoweringError::Unsupported(
                    "computed primitive borrow lost its established local",
                ))?;
                if !valid_structural_root(root)
                    || locals.next().is_some()
                    || !permits(source.argument.access, access)
                    || self.structural_argument_type(&source.argument)
                        != Some(source.structural_type)
                {
                    return unsupported("computed primitive borrow changes its local custody");
                }
                (source.argument.clone(), source.structural_type)
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
                if (parameter.access != StructuralAccess::Owned
                    && parameter.multiplicity != StructuralMultiplicity::Unrestricted)
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                    || !permits(parameter.access, access)
                {
                    return unsupported("computed primitive borrow widens parameter custody");
                }
                (
                    StructuralArgument {
                        place: parameter.place,
                        path: Vec::new(),
                        access: parameter.access,
                    },
                    parameter.structural_type,
                )
            }
            _ => return unsupported("computed primitive borrow has no supported source place"),
        };
        Ok(
            crate::emission::operation_emission::calls::PreparedStructuralArgument {
                place: source.place,
                path: source
                    .path
                    .into_iter()
                    .map(structural_paths::PreparedStructuralPathSegment::Static)
                    .chain(structural_paths::prepare_structural_argument_path(
                        argument,
                        root_type,
                        structural_types,
                    )?)
                    .collect(),
                selector_positions: Vec::new(),
                access,
            },
        )
    }

    /// Argument preparation may add temporary results after the caller's
    /// published storage. Their ordinals share one namespace and cannot alias.
    pub(crate) fn owned_call_argument(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        results: &[(
            u32,
            crate::emission::operation_emission::buffer::StructuralBinding,
        )],
    ) -> Result<StructuralArgument, LoweringError> {
        let Some(ordinal) = argument.source_structural_result_binding_ordinal() else {
            return self.owned_argument(argument);
        };
        let Some((_, binding)) = results.iter().find(|(candidate, _)| *candidate == ordinal) else {
            return self.owned_argument(argument);
        };
        // A prepared result is consumed whole; this lane has no route for an
        // owned projection out of it.
        if !argument.path.is_empty() {
            return unsupported("computed owned operand cannot project a prepared call result");
        }
        Ok(StructuralArgument {
            place: binding.place(),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        })
    }

    /// Resolve a source-validated owned home without inventing a loan. An
    /// established value answers at its own place; a projected owned alias
    /// answers at its root's place under the alias path. Transferring an
    /// owned child consumes that subtree's custody and preserves its
    /// siblings, so naming the alias path is the authorized reading of a
    /// whole-value use rather than a widening.
    pub(crate) fn owned_argument(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    ) -> Result<StructuralArgument, LoweringError> {
        let root = argument.structural_storage_root();
        let ordinal = argument.source_structural_result_binding_ordinal();
        if root.is_some() || ordinal.is_some() {
            let mut locals = self.structural_locals.iter().filter(|row| {
                root.is_some_and(|root| row.root == root)
                    || ordinal.is_some_and(|ordinal| row.result_binding_ordinal == Some(ordinal))
            });
            let StructuralLocalBinding {
                argument: source, ..
            } = locals.next().ok_or(LoweringError::Unsupported(
                "computed owned structural operand lost its established local",
            ))?;
            if root.is_some_and(|root| !valid_structural_root(root))
                || locals.next().is_some()
                || !argument.path.is_empty()
                || argument.access != checked_trees::CheckedStructuralAccess::Owned
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

    /// Bind an immutable local's evaluated value at its source ordinal.
    /// Mutable storage is a primitive place, never a rebound SSA value.
    pub(crate) fn append(&mut self, scalar_type: ScalarType, position: usize) {
        self.immutable.push(Some(position));
        self.authored.push(Some(AuthoredScalarBinding {
            value_position: position,
            scalar_type,
            storage_symbol: symbols::SymbolHandle::invalid(),
        }));
    }

    /// The first addressable home replaces reads of this binding, not the
    /// completed values of other bindings that previously copied its contents.
    pub(crate) fn register_local_storage(
        &mut self,
        ordinal: usize,
        symbol: symbols::SymbolHandle,
        scalar_type: ScalarType,
    ) -> Result<(), LoweringError> {
        let position =
            self.parameter_count
                .checked_add(ordinal)
                .ok_or(LoweringError::Unsupported(
                    "addressed scalar local namespace overflow",
                ))?;
        let binding = self
            .authored
            .get_mut(position)
            .and_then(Option::as_mut)
            .ok_or(LoweringError::Unsupported(
                "addressed scalar local has no completed binding",
            ))?;
        if !symbol.is_valid()
            || binding.scalar_type != scalar_type
            || (binding.storage_symbol.is_valid() && binding.storage_symbol != symbol)
        {
            return unsupported("addressed scalar local changes its original binding");
        }
        binding.storage_symbol = symbol;
        Ok(())
    }

    /// Bind a retained scalar result that no source name reads: a call result
    /// an assignment stores. It takes its binding-ordinal slot, but authored
    /// `Local` reads keep counting only declared locals.
    pub(crate) fn append_unnamed_result(&mut self, position: usize) {
        self.immutable.push(Some(position));
    }

    /// Rejoin an immutable local's source ordinal to its already-evaluated
    /// scalar value. Addressable storage must never replay its initializer.
    pub(crate) fn immutable_local_value(
        &self,
        ordinal: usize,
        scalar_type: ScalarType,
    ) -> Result<usize, LoweringError> {
        let position =
            self.parameter_count
                .checked_add(ordinal)
                .ok_or(LoweringError::Unsupported(
                    "immutable local namespace overflow",
                ))?;
        let binding =
            self.authored
                .get(position)
                .copied()
                .flatten()
                .ok_or(LoweringError::Unsupported(
                    "immutable local has no established scalar value",
                ))?;
        if binding.scalar_type != scalar_type {
            return unsupported("immutable local addressable home changes its scalar type");
        }
        Ok(binding.value_position)
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
                element_path,
                ..
            } => {
                self.scalar(&mut index.expression)?;
                for step in element_path {
                    if let checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) = step
                    {
                        self.scalar(&mut selector.expression)?;
                    }
                }
            }
            CheckedScalarExpression::Parameter { position, .. }
            | CheckedScalarExpression::Local { position, .. } => {
                let binding = self.authored.get(*position).copied().flatten().ok_or(
                    LoweringError::Unsupported("scalar read has no authored binding"),
                )?;
                if binding.storage_symbol.is_valid() {
                    let primitive_type =
                        expression
                            .primitive_type()
                            .ok_or(LoweringError::Unsupported(
                                "addressed scalar read lost its primitive type",
                            ))?;
                    *expression = CheckedScalarExpression::StorageRead {
                        symbol: binding.storage_symbol,
                        primitive_type,
                    };
                } else {
                    *position = binding.value_position;
                }
            }
            CheckedScalarExpression::StorageRead { symbol, .. } => {
                if self
                    .place_observations
                    .iter()
                    .any(|row| row.symbol == *symbol)
                    || self.primitive_storage.iter().any(|row| row.0 == *symbol)
                    || self
                        .structural_locals
                        .iter()
                        .any(|row| row.root == facts::PlaceRoot::Symbol(*symbol))
                {
                    // Retain the read occurrence for emission, not an entry snapshot.
                    return Ok(());
                }
                return unsupported("scalar storage read names no established storage");
            }
            CheckedScalarExpression::IntegerBinary { left, right, .. } => {
                self.scalar(left)?;
                self.scalar(right)?;
            }
            CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
            | CheckedScalarExpression::IntegerWiden { operand, .. }
            | CheckedScalarExpression::RepresentationView { operand, .. }
            | CheckedScalarExpression::IntegerExactCast { operand, .. } => self.scalar(operand)?,
            CheckedScalarExpression::Boolean(expression) => self.boolean(expression)?,
            CheckedScalarExpression::IntegerLiteral { .. }
            | CheckedScalarExpression::StructuralParameterByteLength { .. }
            | CheckedScalarExpression::IeeeFloatLiteral { .. }
            | CheckedScalarExpression::ImplementationSelection { .. }
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
                let binding = self.authored.get(*position).copied().flatten().ok_or(
                    LoweringError::Unsupported("Boolean read has no authored binding"),
                )?;
                if binding.storage_symbol.is_valid() {
                    if binding.scalar_type != ScalarType::Boolean {
                        return unsupported("addressed Boolean read changes its scalar type");
                    }
                    *expression = CheckedBooleanExpression::StorageRead {
                        symbol: binding.storage_symbol,
                    };
                } else {
                    *position = binding.value_position;
                }
            }
            CheckedBooleanExpression::StorageRead { symbol } => {
                if self
                    .place_observations
                    .iter()
                    .any(|row| row.symbol == *symbol)
                    || self.primitive_storage.iter().any(|row| row.0 == *symbol)
                    || self
                        .structural_locals
                        .iter()
                        .any(|row| row.root == facts::PlaceRoot::Symbol(*symbol))
                {
                    return Ok(());
                }
                return unsupported("Boolean storage read names no established storage");
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
            // Predicate paths retain authored selector coordinates. The
            // shared projection emitter reads their current value/home once;
            // remapping here would both remap twice and freeze addressed locals.
            CheckedBooleanExpression::ByteSequencePredicate { .. } => {}
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
        let (_, expression) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(state, statement, role)
            .ok_or(LoweringError::Unsupported(
                "scalar computation needs one checked expression and one source binding",
            ))?;
        self.expression(&checked.facts.proven_scalar_indexes, expression)
    }

    /// A selected dispatch copied this scalar payload before the arm began.
    /// The returned position is already in the emitted value namespace.
    pub(crate) fn established_case_payload(
        &self,
        source: PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
        case: semantic_vocabulary::StructuralCaseId,
        field: semantic_vocabulary::StructuralFieldId,
        scalar_type: ScalarType,
    ) -> Result<Option<LoweredDirectExpression>, LoweringError> {
        Ok(self
            .structural_fields
            .established_payload(source, path, case, field)?
            .map(|position| LoweredDirectExpression::Parameter {
                position,
                scalar_type,
            }))
    }

    /// Observe a checked index field at its authored operand position. The call
    /// expansion carries this value across later argument effects.
    pub(crate) fn runtime_index_field(
        &self,
        field: &checked_trees::CheckedStructuralParameterField,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let observation = structural_fields::resolve_typed(
            &self.structural_fields,
            field.parameter_position,
            &field.path,
        )?;
        if !matches!(observation.scalar_type(), ScalarType::Integer(integer) if !integer.is_address())
        {
            return unsupported("runtime index field selector is not an integer read");
        }
        Ok(observation)
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
        let binding = self
            .authored
            .get(position as usize)
            .copied()
            .flatten()
            .ok_or(LoweringError::Unsupported(
                "runtime selector has no established immutable scalar",
            ))?;
        if !matches!(binding.scalar_type, ScalarType::Integer(integer) if !integer.is_address()) {
            return unsupported("runtime selector is not an integer");
        }
        if binding.storage_symbol.is_valid() {
            let (source, path) = primitive_storage_read(
                &self.primitive_storage,
                &self.structural_locals,
                self.structural_fields.declarations(),
                binding.storage_symbol,
                binding.scalar_type,
            )?;
            Ok(LoweredDirectExpression::PrimitiveRead {
                source: source.into(),
                path,
                scalar_type: binding.scalar_type,
            })
        } else {
            Ok(LoweredDirectExpression::Parameter {
                position: binding.value_position,
                scalar_type: binding.scalar_type,
            })
        }
    }

    pub(crate) fn expression(
        &self,
        proven_indexes: &[checked_trees::expression::ExpressionHandle],
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
            proven_indexes,
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
            let shape = find_structural_type(types, parameter.structural_type)?;
            let terminal_psi::StructuralTypeShape::ElementView { element } = shape.shape else {
                return None;
            };
            let element = find_structural_type(types, element)?;
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
