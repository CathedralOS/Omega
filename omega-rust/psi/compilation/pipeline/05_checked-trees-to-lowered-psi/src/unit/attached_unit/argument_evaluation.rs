//! Scalar operand evaluation within an existing machine's structural frontier.
//!
//! Runtime values, erased formals and captured selectors occupy separate
//! namespaces even though all are value-declaration slices. Expansions bind
//! against the runtime prefix; proof-only actuals read the erased roster. The
//! value, block, edge and place allocators likewise remain distinct through
//! nested emission. Sharing the identity table changes neither correspondence.
use super::super::{
    BlockId, QualifiedScalarType, StructuralAccess, StructuralParameterDeclaration,
    StructuralTypeDeclaration, ValueId, block_id, scalar_carriers,
};
use super::{
    Block, CheckedScalarExpressionRole, CheckedTrees, CheckedUnitEffectOperationPlan,
    LoweringError, Operation, OperationKind, OperationResult, PlaceId, ScalarType,
    StructuralMultiplicity, StructuralTypeShape, Terminator, ValueDeclaration, allocate_dense,
    direct_expression_contains_short_circuit, edge_id, emit_direct_expression,
    terminal_scalar_type, unsupported, validate_direct_parameter_types, value_id,
};
use crate::emission::borrowed_window::terminal_multiplicity;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::buffer::{OperationBuffer, StructuralBinding};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::scalar_lowering::branch_states::prepared::scalar_state_emission_order;
use crate::scalar_lowering::contracts::erased_proof_formal_declarations;
use checked_trees::CheckedCallScalarArgument;
use state_emission::emit_state;

pub(crate) mod cleanup_control;
mod qualification_edge;
mod record_captures;
mod scalar_control;
mod source_values;
mod state_emission;
mod straight_line;

pub(crate) use scalar_control::ScalarReturnCleanup;

/// A completed value retains its physical home and captured projection. The
/// selected type interprets an authored suffix; it is not the home's type.
/// Consumers select this subtree while preserving its siblings' custody.
pub(crate) struct StructuralValueStorage {
    pub(crate) place: PlaceId,
    pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
    pub(crate) selected_type: semantic_vocabulary::StructuralTypeId,
}

impl From<&StructuralBinding> for StructuralValueStorage {
    fn from(binding: &StructuralBinding) -> Self {
        Self {
            place: binding.place(),
            path: binding.path().to_vec(),
            selected_type: binding.structural_type(),
        }
    }
}

pub(crate) struct Evaluation {
    /// The enclosing emission unit's scalar qualification namespace. A call
    /// result publishes the callee's qualified result in it, so argument
    /// evaluation resolves operand memberships against the same catalog the
    /// emitted machine declares — never a machine-private interning.
    pub(crate) shared_qualifications:
        crate::expression_preparation::qualifications::PreparedScalarQualifications,
    /// The emitted machine's terminal identity, stamped on qualification
    /// coercions this evaluation's expansions mint. Emission frames that
    /// cannot mint qualification edges carry none.
    pub(crate) machine: Option<semantic_vocabulary::MachineId>,
    /// Qualification coercions this evaluation's expansions minted; the
    /// machine's emission drains them into the module catalog.
    pub(crate) qualification_coercions: Vec<terminal_psi::ScalarQualificationCoercion>,
    pub(crate) structural_types: std::sync::Arc<[StructuralTypeDeclaration]>,
    pub(crate) structural_value_owners: Vec<StructuralValueOwner>,
    pub(crate) selection_cleanups: Vec<SelectionCleanup>,
    pub(crate) structural_locals:
        Vec<crate::expression_preparation::bindings::StructuralLocalBinding>,
    /// The view family of each established immutable view local, so scalar
    /// observations rooted at one pick its length and read operations.
    pub(crate) view_locals:
        Vec<crate::expression_preparation::bindings::view_locals::ViewLocalBinding>,
    /// Element views among the structural parameters, resolved once where the
    /// parameters and type roster are bound, so every namespace this
    /// evaluation builds reads and measures an element-view parameter with
    /// element operations.
    pub(crate) element_views:
        std::collections::BTreeMap<semantic_vocabulary::StructuralTypeId, Option<ScalarType>>,
    pub(crate) subslices: Vec<crate::scalar_lowering::computations::subslices::Slot>,
    pub(crate) observations: Vec<crate::scalar_lowering::computations::observations::Slot>,
    pub(crate) produced_values: Vec<crate::scalar_lowering::computations::produced_values::Slot>,
    pub(crate) literals: Vec<crate::scalar_lowering::computations::literals::Slot>,
    pub(crate) copies: Vec<crate::scalar_lowering::computations::copies::Slot>,
    pub(crate) cases: Vec<crate::scalar_lowering::computations::cases::Slot>,
    pub(crate) record_fields: Vec<crate::scalar_lowering::computations::fields::Binding>,
    pub(crate) place_observations: std::sync::Arc<
        [crate::expression_preparation::bindings::place_observations::PlaceObservation],
    >,
    /// Checked computation roots indexed once per authored state, then borrowed
    /// by local statement coordinate when original backing is demanded.
    pub(crate) computation_address_roots: Vec<(
        symbols::SymbolHandle,
        Vec<(u32, checked_trees::CheckedScalarComputationHandle)>,
    )>,
    /// A composed state's `mut` scalar formal storage. Every operation
    /// sequence the state emits stores into it by the formal's symbol, as
    /// it does into a `let mut` local the sequence establishes.
    pub(crate) parameter_storage: Vec<super::primitive_locals::PrimitiveLocal>,
    pub primitive_storage: Vec<(symbols::SymbolHandle, PlaceId, ScalarType)>,
    /// The body's scalar binding namespace: authored formals and locals on
    /// their dense values. Each route installs its entry namespace before it
    /// emits a statement; state-local storage has its own rows in it.
    pub scalar_bindings: crate::expression_preparation::bindings::ScalarBindings,
    /// Current formal bindings, keyed by authored position. Owned replacement
    /// changes this source namespace, never the machine's entry declarations.
    pub structural_parameters: Vec<(u32, StructuralParameterDeclaration)>,
    /// Earlier formal homes remain declaration geometry for captured aliases,
    /// but no longer resolve a new occurrence of the formal's source name.
    pub(crate) previous_parameter_homes: Vec<StructuralParameterDeclaration>,
    pub structural_fields: crate::expression_preparation::bindings::StructuralFieldBindings,
    pub structural_cases:
        Vec<crate::expression_preparation::bindings::structural_cases::StructuralCaseBinding>,
    /// The enclosing signature's erased scalar formals, for `ErasedParameter`
    /// positions inside proof-only call actuals lowered in this namespace.
    pub erased_scalar_formals: Vec<ValueDeclaration>,
    /// The enclosing signature's erased proof-only roster, in checked order.
    /// Every emitted block redeclares it and every internal edge forwards it,
    /// so `ProofTerm::Formal` positions stay in scope across the evaluation's
    /// private joins.
    pub erased_proof_formals: Vec<checked_trees::CheckedErasedProofParameterPlan>,
    pub entry: BlockId,
    pub current: BlockId,
    pub parameters: Vec<ValueDeclaration>,
    /// Parameters established at the current private join, distinct from the
    /// authored structural namespace used to resolve source operands.
    pub block_structural_parameters: Vec<StructuralParameterDeclaration>,
    pub operation_start: usize,
    pub blocks: Vec<Block>,
}

/// Replace candidate slots only at their normal death edge. The join retains
/// each source slot's residual parameter; a rebind can change establishment
/// order without changing the historical producer roster. This is slot
/// correspondence, not a fixed donor identity for an alternative residual.
/// An ordinary move has no residual candidates and only rebinds survivors.
#[derive(Clone)]
pub(crate) struct SelectionCleanup {
    pub(crate) selection: arena::Handle<checked_trees::FlowOwnedSelectionReceipt>,
    pub(crate) selected: PlaceId,
    pub(crate) sources: Vec<PlaceId>,
    pub(crate) remaining: Vec<(PlaceId, PlaceId)>,
    pub(crate) retired_reference_paths: Vec<(PlaceId, Vec<terminal_psi::StructuralPathSegment>)>,
    pub(crate) pass_through: Vec<(PlaceId, PlaceId)>,
    pub(crate) next_operation: u64,
}

/// Current physical owners in establishment order. An anonymous residual keeps
/// its receipt correspondence in SelectionCleanup, never a fabricated symbol.
#[derive(Clone)]
pub(crate) struct StructuralValueOwner {
    pub(crate) symbol: symbols::SymbolHandle,
    pub(crate) statement: u32,
    pub(crate) value: terminal_psi::StructuralOperationResult,
}

impl Evaluation {
    /// The evaluation's scalar bindings extended with the storage, local,
    /// parameter and observation context every lowering walker needs.
    pub(crate) fn contextual_bindings(
        &self,
    ) -> crate::expression_preparation::bindings::ScalarBindings {
        self.scalar_bindings
            .clone()
            .with_primitive_storage(&self.primitive_storage)
            .with_structural_locals(&self.structural_locals)
            .with_view_locals(&self.view_locals)
            .with_element_views(&self.element_views)
            .with_structural_parameters(&self.structural_parameters)
            .with_resolved_structural_observations(&self.structural_fields, &self.structural_cases)
            .with_place_observations(&self.place_observations)
    }

    /// Observe completed producer homes before final machine assembly publishes
    /// their declarations. The operation result is the exact type/producer
    /// owner; this inventory supplies geometry, never storage liveness or
    /// permission to consume a value. A producer may have completed in one of
    /// this evaluation's private blocks rather than the current buffer.
    pub(crate) fn completed_bindings(
        &self,
        operations: &OperationBuffer,
        declarations: impl IntoIterator<Item = terminal_psi::StructuralPlaceDeclaration>,
    ) -> crate::expression_preparation::bindings::ScalarBindings {
        self.contextual_bindings()
            .with_completed_place_declarations(
                declarations.into_iter().chain(
                    crate::emission::operation_emission::place_declarations::operation_results(
                        operations
                            .iter()
                            .chain(self.blocks.iter().flat_map(|block| block.operations.iter())),
                    ),
                ),
            )
    }

    /// Selected cleanup sources that are declared parameters, in descending
    /// authored-position order and deduplicated by place.

    /// A full scalar-computation expansion populated from this
    /// evaluation's prepared tables.
    fn scalar_computation_expansion<'a>(
        &'a self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &'a CheckedTrees,
        qualifications: &'a crate::expression_preparation::qualifications::PreparedScalarQualifications,
        machine: symbols::SymbolHandle,
        operations: &OperationBuffer,
    ) -> Result<crate::scalar_lowering::computations::Expansion<'a>, LoweringError> {
        Ok(crate::scalar_lowering::computations::Expansion::new(
            checked,
            qualifications,
            machine,
            1,
        )
        .with_structural_types(&self.structural_types)
        .with_produced_values(&self.produced_values)
        .with_literals(&self.literals)
        .with_copies(&self.copies)
        .with_subslices(&self.subslices)
        .with_observations(&self.observations)
        .with_source_carriers(self.completed_carrier_inventory(operations)?)
        .with_cases(&self.cases)
        .with_fields(&self.record_fields)
        .enter_proof_scope(identities, &self.erased_proof_formals))
    }

    pub(crate) fn cleanup_parameter_places(&self) -> Vec<PlaceId> {
        let mut parameter_sources = Vec::new();
        for cleanup in &self.selection_cleanups {
            for source in &cleanup.sources {
                if let Some((position, _)) = self
                    .structural_parameters
                    .iter()
                    .find(|(_, declaration)| declaration.place == *source)
                {
                    parameter_sources.push((*position, *source));
                }
            }
        }
        parameter_sources.sort_by_key(|(position, _)| std::cmp::Reverse(*position));
        parameter_sources.dedup_by_key(|(_, place)| *place);
        parameter_sources
            .into_iter()
            .map(|(_, place)| place)
            .collect()
    }

    /// Resolve a completed value's storage and selected subtree. Retirement
    /// and replacement share this identity join. Result registries differ
    /// between ordinary and graph bodies; the caller supplies its registry.
    pub(crate) fn value_storage(
        &self,
        source: &checked_trees::CheckedUnitStructuralArgumentSourcePlan,
        parameters: &[StructuralParameterDeclaration],
        mut result: impl FnMut(u32) -> Result<StructuralValueStorage, LoweringError>,
    ) -> Result<StructuralValueStorage, LoweringError> {
        use checked_trees::CheckedUnitStructuralArgumentSourcePlan;
        let mut storage = match *source {
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                let parameter = self
                    .structural_parameters
                    .get(parameter_index as usize)
                    .map(|(_, parameter)| parameter)
                    .or_else(|| parameters.get(parameter_index as usize))
                    .ok_or(LoweringError::Unsupported(
                        "value storage parameter is absent",
                    ))?;
                StructuralValueStorage {
                    place: parameter.place,
                    path: Vec::new(),
                    selected_type: parameter.structural_type,
                }
            }
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. } => {
                let root = match *source {
                    CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                        facts::PlaceRoot::Symbol(symbol)
                    }
                    CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        expression,
                    } => facts::PlaceRoot::Expression(expression),
                    _ => return unsupported("value storage source has no storage identity"),
                };
                let local = crate::expression_preparation::bindings::structural_local_by_root(
                    &self.structural_locals,
                    root,
                )
                .ok_or(LoweringError::Unsupported(
                    "value storage local is not established",
                ))?;
                StructuralValueStorage {
                    place: local.argument.place,
                    path: local.argument.path.clone(),
                    selected_type: local.structural_type,
                }
            }
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                result(binding_ordinal)?
            }
            _ => return unsupported("value storage source has no owned structural storage"),
        };
        storage.place = self.current_structural_place(storage.place);
        Ok(storage)
    }

    /// Bind the emitted structural signature before evaluating any operands.
    /// Locals may name transported parameter homes rather than operation
    /// results. Resolve their exact declarations without replaying a producer.
    pub(crate) fn structural_root_parameters(
        &self,
    ) -> impl Iterator<Item = &StructuralParameterDeclaration> + Clone {
        self.structural_parameters
            .iter()
            .map(|(_, declaration)| declaration)
            .chain(self.previous_parameter_homes.iter())
            .chain(self.block_structural_parameters.iter())
            .chain(
                self.blocks
                    .iter()
                    .flat_map(|block| block.structural_parameters.iter()),
            )
    }

    /// Snapshot the current source namespace for an operation. This changes
    /// bindings only: already captured operands keep their historical homes.
    pub(crate) fn current_parameters<'parameters>(
        &self,
        declared: &'parameters [StructuralParameterDeclaration],
    ) -> std::borrow::Cow<'parameters, [StructuralParameterDeclaration]> {
        if self.previous_parameter_homes.is_empty() {
            std::borrow::Cow::Borrowed(declared)
        } else {
            std::borrow::Cow::Owned(
                self.structural_parameters
                    .iter()
                    .map(|(_, parameter)| parameter.clone())
                    .collect(),
            )
        }
    }

    /// Field, case and extent observations share this exact source-position
    /// roster; a boundary-return body has the same namespace as an ordinary
    /// body even when its foreign invocation accepts only scalar operands.
    pub(crate) fn bind_structural_parameters(
        &mut self,
        parameters: Vec<(u32, StructuralParameterDeclaration)>,
        structural_types: &[StructuralTypeDeclaration],
    ) {
        self.structural_types = structural_types.into();
        self.structural_fields =
            crate::expression_preparation::bindings::StructuralScalarFieldBinding::collect(
                &parameters,
                structural_types,
            );
        self.structural_cases =
            crate::expression_preparation::bindings::structural_cases::StructuralCaseBinding::collect(
                &parameters,
                structural_types,
            );
        self.element_views =
            crate::expression_preparation::bindings::element_views(&parameters, structural_types);
        self.structural_parameters = parameters;
    }

    /// Start a mutually exclusive branch with the current source namespace.
    /// Completed blocks and their operations remain with the parent; only the
    /// incoming frontier is shared. Each branch can then move or rebind owners
    /// without changing the sibling's compile-time view of that frontier.
    pub(crate) fn branch(&self, block: BlockId, operation_start: usize) -> Self {
        Self {
            shared_qualifications: self.shared_qualifications.clone(),
            machine: self.machine,
            qualification_coercions: self.qualification_coercions.clone(),
            structural_types: self.structural_types.clone(),
            structural_value_owners: self.structural_value_owners.clone(),
            selection_cleanups: self.selection_cleanups.clone(),
            structural_locals: self.structural_locals.clone(),
            view_locals: self.view_locals.clone(),
            element_views: self.element_views.clone(),
            produced_values: self.produced_values.clone(),
            literals: self.literals.clone(),
            copies: self.copies.clone(),
            subslices: self.subslices.clone(),
            observations: self.observations.clone(),
            cases: self.cases.clone(),
            record_fields: self.record_fields.clone(),
            place_observations: self.place_observations.clone(),
            computation_address_roots: self.computation_address_roots.clone(),
            primitive_storage: self.primitive_storage.clone(),
            parameter_storage: self.parameter_storage.clone(),
            scalar_bindings: self.scalar_bindings.clone(),
            structural_parameters: self.structural_parameters.clone(),
            previous_parameter_homes: self.previous_parameter_homes.clone(),
            structural_fields: self.structural_fields.clone(),
            structural_cases: self.structural_cases.clone(),
            erased_scalar_formals: self.erased_scalar_formals.clone(),
            erased_proof_formals: self.erased_proof_formals.clone(),
            entry: self.entry,
            current: block,
            parameters: Vec::new(),
            block_structural_parameters: Vec::new(),
            operation_start,
            blocks: Vec::new(),
        }
    }

    /// Keep one affine owner for an emitted home. A later named binding claims
    /// the anonymous call row; two different locals on one place are refused.
    fn record_affine_home(&mut self, owner: StructuralValueOwner) -> Result<(), LoweringError> {
        if let Some(existing) = self
            .structural_value_owners
            .iter_mut()
            .find(|existing| existing.value.place == owner.value.place)
        {
            if existing.symbol.is_valid()
                && owner.symbol.is_valid()
                && existing.symbol != owner.symbol
            {
                return unsupported("affine home is owned by two locals");
            }
            if !existing.symbol.is_valid() && owner.symbol.is_valid() {
                existing.symbol = owner.symbol;
                existing.statement = owner.statement;
            }
            return Ok(());
        }
        self.structural_value_owners.push(owner);
        Ok(())
    }

    /// Publish one completed structural result into the state's existing value
    /// and local namespaces. Calls and constructors share the original home;
    /// registration neither copies payload fields nor changes cleanup custody.
    /// The plan's binding names the local it initializes: nested operands
    /// share its statement coordinate but own no source local.
    pub(crate) fn establish_structural_result(
        &mut self,
        state: symbols::SymbolHandle,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        produced: terminal_psi::StructuralOperationResult,
        structural_types: &[StructuralTypeDeclaration],
        operations: &mut OperationBuffer,
    ) -> Result<(), LoweringError> {
        let multiplicity = terminal_multiplicity(result.multiplicity);
        if !structural_types
            .iter()
            .any(|declaration| declaration.id == produced.structural_type)
        {
            return unsupported("structural result type was not declared");
        }
        if multiplicity == StructuralMultiplicity::Linear {
            // Registration does not establish authority. Rejoin the exact
            // emitted call result and source occurrence; its checked custody
            // is independent of ordinary versus boundary dispatch.
            let mut producers = operations.iter().filter(|operation| {
                matches!(&operation.result,
                OperationResult::Structural(value) if value == &produced)
            });
            let producer = producers.next().ok_or(LoweringError::Unsupported(
                "linear local has no exact completed call result",
            ))?;
            if producers.next().is_some()
                || produced.claims.is_empty()
                || !operations.source_calls.iter().any(|occurrence| {
                    occurrence.source_state == state
                        && occurrence.statement_index == result.statement_index as usize
                        && occurrence.terminal_site.operation() == Some(producer.id)
                })
            {
                return unsupported("linear local lost its exact call result custody");
            }
        }
        self.establish_structural_binding(result, produced.into(), structural_types, operations)
    }

    /// Bind an already established place to its authored local. Producer-specific
    /// admission happens before this shared namespace and local-symbol step.
    pub(crate) fn establish_structural_binding(
        &mut self,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        produced: StructuralBinding,
        structural_types: &[StructuralTypeDeclaration],
        operations: &mut OperationBuffer,
    ) -> Result<(), LoweringError> {
        let multiplicity = produced.multiplicity();
        let declaration =
            terminal_psi::find_structural_type(structural_types, produced.structural_type())
                .ok_or(LoweringError::Unsupported(
                    "structural binding type was not declared",
                ))?;
        if operations
            .structural_values
            .iter()
            .any(|(ordinal, _)| *ordinal == result.binding_ordinal)
        {
            return unsupported("structural value binding was established twice");
        }
        let local = result.local;
        if local.is_named() {
            if self
                .structural_locals
                .iter()
                .any(|binding| binding.root == facts::PlaceRoot::Symbol(local.symbol))
            {
                return unsupported("structural value repeats its local binding");
            }
            // A `&T` local holds shared-borrow custody of the referent's
            // storage and a `&mut` view local exclusive loan custody, even
            // though the borrow shell's own type multiplicity spells
            // Unrestricted. A materialized Reference carrier instead owns its
            // descriptor: borrow access belongs to the explicit Referent
            // crossing, not that home.
            let access = crate::expression_preparation::bindings::structural_paths::terminal_access(
                local.access,
            );
            let reference_carrier =
                matches!(declaration.shape, StructuralTypeShape::Reference { .. });
            self.structural_fields
                .adopt_declarations(produced.structural_type(), structural_types);
            self.structural_locals.push(
                crate::expression_preparation::bindings::StructuralLocalBinding {
                    result_binding_ordinal: Some(result.binding_ordinal),
                    root: facts::PlaceRoot::Symbol(local.symbol),
                    structural_type: produced.structural_type(),
                    argument:
                        crate::expression_preparation::bindings::structural_paths::structural_argument(
                            produced.place(),
                            produced.path().to_vec(),
                            if reference_carrier {
                                StructuralAccess::Owned
                            } else {
                                access
                            },
                        ),
                },
            );
            // A view local's published place is what its lengths and element
            // reads observe; record the family its produced type declares.
            // A view observes the place it names; an alias names a subtree
            // of another place, which this roster cannot express.
            if produced.path().is_empty()
                && let Some(carrier) =
                    crate::expression_preparation::bindings::view_locals::ViewCarrier::of(
                        produced.structural_type(),
                        structural_types,
                    )
            {
                self.view_locals.push(
                    crate::expression_preparation::bindings::view_locals::ViewLocalBinding {
                        symbol: local.symbol,
                        place: produced.place(),
                        structural_type: produced.structural_type(),
                        carrier,
                        access,
                    },
                );
            }
        }
        // Residual cleanup tracks values this statement established. A
        // projected owned alias established none: the root it names keeps its
        // own owner row and its own residual complement.
        // A call and the local initialized from it publish one emitted home.
        // The anonymous result and the named binding are two registrations of
        // that home. A second row would make the next edge transfer the owner
        // twice, so the later named binding claims the existing row.
        if multiplicity == StructuralMultiplicity::Affine && produced.path().is_empty() {
            self.record_affine_home(StructuralValueOwner {
                symbol: local.symbol,
                statement: result.statement_index,
                value: produced.operation_result()?.clone(),
            })?;
        }
        operations.bind_structural_value(result.binding_ordinal, produced);
        Ok(())
    }

    /// Call operand resolvers retain authored result identities. Apply only
    /// transports that precede the operation; earlier calls keep their places.
    pub(crate) fn remap_transported_call_operands(&mut self, operations: &mut OperationBuffer) {
        let cleanups = &self.selection_cleanups;
        let remap = |operation: &mut Operation| {
            let Some(arguments) = operation.kind.structural_arguments_mut() else {
                return;
            };
            for cleanup in cleanups {
                if operation.id.get() < cleanup.next_operation {
                    continue;
                }
                for argument in arguments.iter_mut() {
                    if let Some((_, target)) = cleanup
                        .pass_through
                        .iter()
                        .find(|(source, _)| *source == argument.place)
                    {
                        argument.place = *target;
                    }
                }
            }
        };
        for operation in &mut operations.operations {
            remap(operation);
        }
        if let Some(completion) = &mut operations.external_completion {
            for cleanup in cleanups {
                for argument in &mut completion.structural_arguments {
                    if let Some((_, target)) = cleanup
                        .pass_through
                        .iter()
                        .find(|(source, _)| *source == argument.place)
                    {
                        argument.place = *target;
                    }
                }
            }
        }
        for block in &mut self.blocks {
            for operation in &mut block.operations {
                remap(operation);
            }
        }
    }

    pub(crate) fn current_structural_place(&self, mut place: PlaceId) -> PlaceId {
        for cleanup in &self.selection_cleanups {
            if let Some((_, target)) = cleanup
                .pass_through
                .iter()
                .find(|(source, _)| *source == place)
            {
                place = *target;
            }
        }
        place
    }

    /// Remove a retired physical occurrence from current source and transport
    /// rosters. Historical declarations and shared type geometry stay intact.
    pub(crate) fn retire_structural_home(&mut self, place: PlaceId) {
        self.structural_value_owners
            .retain(|owner| owner.value.place != place);
        self.structural_locals
            .retain(|local| local.argument.place != place);
        let mut views = std::mem::take(&mut self.view_locals);
        views.retain(|view| self.current_structural_place(view.place) != place);
        self.view_locals = views;
    }

    pub(crate) fn selection_return_discards(
        &self,
        mut roots: Vec<(PlaceId, bool)>,
    ) -> Result<Vec<PlaceId>, LoweringError> {
        for cleanup in &self.selection_cleanups {
            if cleanup.sources.is_empty() {
                if !cleanup.remaining.is_empty() {
                    return unsupported("owned move cannot fabricate residual cleanup roots");
                }
                // An ordinary move only transports surviving owners. Its
                // result can leave directly from a return expression, without
                // an operation-roster slot; no candidate row is substituted.
                for (place, _) in &mut roots {
                    if let Some((_, target)) = cleanup
                        .pass_through
                        .iter()
                        .find(|(source, _)| source == place)
                    {
                        *place = *target;
                    }
                }
                continue;
            }
            let selected = roots
                .iter()
                .position(|(place, _)| *place == cleanup.selected)
                .ok_or(LoweringError::Unsupported(
                    "selected cleanup result is absent from its operation roster",
                ))?;
            let start = roots
                .iter()
                .position(|(place, _)| cleanup.sources.contains(place))
                .ok_or(LoweringError::Unsupported(
                    "owned selection normal cleanup has no source roster",
                ))?;
            let end = roots
                .iter()
                .rposition(|(place, _)| cleanup.sources.contains(place))
                .ok_or(LoweringError::Unsupported(
                    "owned selection cleanup source is absent",
                ))?
                + 1;
            if start <= selected
                || cleanup
                    .sources
                    .iter()
                    .any(|source| roots.iter().filter(|(place, _)| place == source).count() != 1)
                || cleanup.remaining.len() + 1 != cleanup.sources.len()
                || cleanup.remaining.iter().any(|(source, _)| {
                    !cleanup.sources.contains(source)
                        || cleanup
                            .remaining
                            .iter()
                            .filter(|(other, _)| other == source)
                            .count()
                            != 1
                })
            {
                return unsupported(
                    "owned selection cleanup source roster does not match its receipt",
                );
            }
            if roots[start..end]
                .iter()
                .any(|(place, discard)| cleanup.sources.contains(place) && *discard)
            {
                return unsupported("owned selection source retains conflicting return disposal");
            }
            // Replace slots by their recorded join correspondence, never by
            // zipping declaration order with current storage order. Interleaved
            // pass-through owners retain their own rows; the displaced slot
            // has no residual because the selected result owns that position.
            let replacement = roots[start..end]
                .iter()
                .filter_map(|(place, discard)| {
                    if cleanup.sources.contains(place) {
                        return cleanup
                            .remaining
                            .iter()
                            .find(|(source, _)| source == place)
                            .map(|(_, receiving)| (*receiving, true));
                    }
                    Some((
                        cleanup
                            .pass_through
                            .iter()
                            .find(|(source, _)| source == place)
                            .map(|(_, target)| *target)
                            .unwrap_or(*place),
                        *discard,
                    ))
                })
                .collect::<Vec<_>>();
            roots.splice(start..end, replacement);
            for (place, _) in &mut roots {
                if let Some((_, target)) = cleanup
                    .pass_through
                    .iter()
                    .find(|(source, _)| source == place)
                {
                    *place = *target;
                }
            }
        }
        Ok(roots
            .into_iter()
            .filter_map(|(place, discard)| discard.then_some(place))
            .collect())
    }

    /// Preserve evaluated scalar bindings while committing only the dying
    /// affine owners on the completed call's normal continuation.
    pub(crate) fn continue_values(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        mut cleanup_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &OperationBuffer,
    ) -> Result<semantic_vocabulary::EdgeId, LoweringError> {
        for action in &mut cleanup_actions {
            let place = match action {
                terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place)
                | terminal_psi::TerminalAffineCleanupAction::RetireHome(place) => place,
                terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                    &mut discard.place
                }
                terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                    &mut cleanup.place
                }
            };
            *place = self.current_structural_place(*place);
        }
        self.cleanup_emission(checked, &mut Vec::new())
            .continue_actions(
                identities,
                cleanup_actions,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
            )
    }

    /// A local refinement is proved where its initializer has completed. The
    /// ordinary continuation carries the exact evaluated values; its arrival
    /// obligation precedes every use of the newly established local.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn require_value_refinements(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        machine: semantic_vocabulary::MachineId,
        predicates: Vec<checked_trees::CheckedBooleanExpression>,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<(), LoweringError> {
        if predicates.is_empty() {
            return Ok(());
        }
        let edge = self.continue_values(
            identities,
            checked,
            Vec::new(),
            values,
            next_value,
            next_block,
            next_edge,
            operations,
        )?;
        let clauses = predicates
            .into_iter()
            .map(|predicate| {
                Some(checked_trees::ClosedScalarContractValue::Predicate(
                    predicate,
                ))
            })
            .collect::<Vec<_>>();
        let predicate = crate::scalar_lowering::contracts::clauses(
            identities,
            &clauses,
            values,
            &self.erased_scalar_formals,
            &crate::scalar_lowering::contracts::ContractViewNamespace::EMPTY,
        )?
        .ok_or(LoweringError::Unsupported(
            "local refinement lost its predicate",
        ))?;
        operations
            .scalar_block_invariants
            .push(terminal_psi::ScalarBlockInvariant {
                machine,
                header: self.current,
                predicate,
                arrivals: vec![terminal_psi::ScalarBlockInvariantArrival {
                    edge,
                    obligation: calls.allocate_requirement()?,
                }],
            });
        Ok(())
    }

    pub(crate) fn new(next_block: &mut u64) -> Result<Self, LoweringError> {
        let entry = block_id(allocate_dense(next_block)?);
        Ok(Self {
            shared_qualifications:
                crate::expression_preparation::qualifications::PreparedScalarQualifications::empty(),
            machine: None,
            qualification_coercions: Vec::new(),
            structural_types: std::sync::Arc::from([]),
            structural_value_owners: Vec::new(),
            selection_cleanups: Vec::new(),
            structural_locals: Vec::new(),
            view_locals: Vec::new(),
            element_views: std::collections::BTreeMap::new(),
            produced_values: Vec::new(),
            literals: Vec::new(),
            copies: Vec::new(),
            subslices: Vec::new(),
            observations: Vec::new(),
            cases: Vec::new(),
            record_fields: Vec::new(),
            place_observations: std::sync::Arc::from([]),
            computation_address_roots: Vec::new(),
            primitive_storage: Vec::new(),
            parameter_storage: Vec::new(),
            scalar_bindings: crate::expression_preparation::bindings::ScalarBindings::new(
                std::iter::empty(),
            ),
            structural_fields: Default::default(),
            structural_cases: Vec::new(),
            structural_parameters: Vec::new(),
            previous_parameter_homes: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            entry,
            current: entry,
            parameters: Vec::new(),
            block_structural_parameters: Vec::new(),
            operation_start: 0,
            blocks: Vec::new(),
        })
    }

    /// Forward the whole erased-proof roster positionally: the identity edge
    /// arguments a private join passes when it redeclares the caller roster.
    /// The forwarding is a lowering-stage term list: it resolves against the
    /// caller's value namespaces wherever the edge is emitted.
    fn proof_formal_forwarding(&self) -> Vec<crate::scalar_lowering::contracts::LoweredProofTerm> {
        (0..self.erased_proof_formals.len())
            .map(
                |position| crate::scalar_lowering::contracts::LoweredProofTerm::Formal {
                    position: u32::try_from(position)
                        .expect("erased-proof roster positions fit u32"),
                },
            )
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn arguments(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        operation: &CheckedUnitEffectOperationPlan,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<Option<Vec<ValueDeclaration>>, LoweringError> {
        self.arguments_slice(
            identities,
            checked,
            machine,
            state,
            operation,
            None,
            values.len(),
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
        )
    }

    /// Evaluate one address operand at its authored argument point. The
    /// schedule retains the result across later effects and private joins;
    /// address emission consumes this value without replaying its source.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn call_selector(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        operation: &CheckedUnitEffectOperationPlan,
        argument: usize,
        segment: usize,
        source_value_count: usize,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        let call = operation
            .call()
            .ok_or(LoweringError::Unsupported("selector has no call"))?;
        let path = call
            .structural_arguments
            .get(argument)
            .and_then(|argument| argument.call_projection().get(segment))
            .ok_or(LoweringError::Unsupported(
                "selector has no argument path occurrence",
            ))?;
        let value = match path {
            checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(root) => {
                let source = checked
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .iter()
                    .map(|(_, source)| source)
                    .find(|source| {
                        source.root == *root
                            && source.state == state
                            && source.statement_ordinal == call.coordinate.statement_index
                    })
                    .ok_or(LoweringError::Unsupported(
                        "call selector lost its source computation",
                    ))?;
                self.source_value(
                    identities,
                    checked,
                    machine,
                    state,
                    source.statement_ordinal,
                    source.role,
                    &CheckedCallScalarArgument::Computation(*root),
                    source_value_count,
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                    calls,
                )?
            }
            checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                index @ (checked_trees::CheckedRuntimeIndex::StructuralField(_)
                | checked_trees::CheckedRuntimeIndex::StorageRead { .. }),
            ) => {
                let bindings = self.contextual_bindings();
                let expression = match index {
                    checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
                        bindings.runtime_index_field(identities, field)?
                    }
                    checked_trees::CheckedRuntimeIndex::StorageRead { symbol } => {
                        bindings.runtime_index_storage(*symbol)?
                    }
                    _ => return unsupported("scheduled storage selector lost its source"),
                };
                let source_types = values
                    .iter()
                    .map(|value| value.value_type())
                    .collect::<Vec<_>>();
                let carrier = expression.value_type(&source_types)?;
                ValueDeclaration {
                    id: emit_direct_expression(&expression, values, next_value, operations),
                    scalar_type: carrier.scalar_type,
                    qualifications: carrier.qualifications,
                }
            }
            _ => return unsupported("scheduled selector has no evaluated operand"),
        };
        if !matches!(value.scalar_type, ScalarType::Integer(integer) if !integer.is_address()) {
            return unsupported("call selector is not an integer operand");
        }
        Ok(value)
    }

    /// Run the recorded window endpoints of one folded structural argument.
    /// The argument's coordinates are already fixed in its checked path, so
    /// no later operation consumes these values; the authored expressions
    /// still execute — their calls and writes are authored effects — in
    /// start-then-end order at the argument's own position.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn window_endpoints(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        operation: &CheckedUnitEffectOperationPlan,
        argument: usize,
        source_value_count: usize,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<(), LoweringError> {
        let Some(call) = operation.call() else {
            return unsupported("window endpoint schedule has no call");
        };
        for (role, root) in super::argument_schedule::recorded_window_endpoint_roots(
            checked,
            state,
            call.coordinate,
            argument,
        ) {
            let value = self.source_value(
                identities,
                checked,
                machine,
                state,
                call.coordinate.statement_index,
                role,
                &CheckedCallScalarArgument::Computation(root),
                source_value_count,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
                calls,
            )?;
            values.push(value);
        }
        Ok(())
    }

    /// Complete one dense scalar operand at its original call coordinate.
    /// The caller retains the result in `values` before evaluating another
    /// operand and uses indices to follow values across private blocks. Only
    /// the pre-group source prefix may be read by authored operand expressions;
    /// later retained slots carry private argument temporaries across the CFG.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn argument_at(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        operation: &CheckedUnitEffectOperationPlan,
        argument_ordinal: usize,
        source_value_count: usize,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        let arguments = self.arguments_slice(
            identities,
            checked,
            machine,
            state,
            operation,
            Some(argument_ordinal),
            source_value_count,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
        )?;
        match arguments.as_deref() {
            Some([argument]) => Ok(*argument),
            _ => unsupported("call has no single completed scalar operand"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn arguments_slice(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        operation: &CheckedUnitEffectOperationPlan,
        argument_ordinal: Option<usize>,
        source_value_count: usize,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<Option<Vec<ValueDeclaration>>, LoweringError> {
        if source_value_count > values.len() {
            return unsupported("call argument source prefix exceeds its retained values");
        }
        let source_bindings = self.contextual_bindings();
        let Some(call) = operation.call() else {
            return Ok(None);
        };
        // Whole-call and staged evaluation use the same dense operand roster:
        // a dynamic dispatch selector is authored first, ahead of the data.
        let operands = call
            .scalar_operands()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let (coordinate, arguments) = (call.coordinate, operands.as_slice());
        let (arguments, argument_ordinal_start) = match argument_ordinal {
            Some(ordinal) => {
                let argument = arguments.get(ordinal).ok_or(LoweringError::Unsupported(
                    "call scalar operand ordinal is outside its argument list",
                ))?;
                (std::slice::from_ref(argument), ordinal)
            }
            None => (arguments, 0),
        };
        let qualifications = &self.shared_qualifications;
        let source_types = values
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        let argument_types = arguments
            .iter()
            .map(|argument| match argument {
                CheckedCallScalarArgument::Pure(expression) => source_bindings
                    .expression(identities, &checked.facts.proven_scalar_indexes, expression)?
                    .value_type(&source_types),
                CheckedCallScalarArgument::Computation(root) => {
                    let nodes = &checked.facts.values.scalar_computations.nodes;
                    if !nodes.is_valid(*root) {
                        return unsupported("call argument computation has no live root");
                    }
                    crate::scalar_lowering::computations::computation_value_type(
                        identities,
                        checked,
                        qualifications,
                        *root,
                        &source_bindings,
                        &source_types,
                    )
                }
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let needs_control = arguments.iter().any(|argument| match argument {
            CheckedCallScalarArgument::Computation(_) => true,
            CheckedCallScalarArgument::Pure(expression) => source_bindings
                .expression(identities, &checked.facts.proven_scalar_indexes, expression)
                .is_ok_and(|expression| direct_expression_contains_short_circuit(&expression)),
        });
        if !needs_control {
            return arguments
                .iter()
                .zip(&argument_types)
                .map(|(argument, scalar_type)| {
                    let expression = source_bindings.expression(
                        identities,
                        &checked.facts.proven_scalar_indexes,
                        argument.as_pure().ok_or(LoweringError::Unsupported(
                            "computed call argument requires ordered control",
                        ))?,
                    )?;
                    validate_direct_parameter_types(&expression, &scalar_carriers(&source_types))?;
                    Ok(ValueDeclaration {
                        qualifications: scalar_type.qualifications,
                        id: emit_direct_expression(&expression, values, next_value, operations),
                        scalar_type: scalar_type.scalar_type,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()
                .map(Some);
        }

        let mut expansion = self.scalar_computation_expansion(
            identities,
            checked,
            qualifications,
            machine,
            operations,
        )?;
        let entry_index = expansion.call_arguments(
            identities,
            state,
            coordinate,
            arguments,
            argument_ordinal_start,
            &source_bindings,
            &source_types,
            0,
        )?;
        let states = expansion.finish();
        self.complete_expansion(
            identities,
            checked,
            &states,
            entry_index,
            &argument_types,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
        )
        .map(Some)
    }

    /// Finish the selected RHS without changing the destination or source slots.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn field_assignment_value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement_index: u32,
        value: &checked_trees::CheckedScalarStoreValue,
        primitive_type: typed_trees::types::PrimitiveType,
        field_identity: Option<&checked_trees::CheckedMemberIdentity>,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        let bindings = self.contextual_bindings();
        let qualifications = &self.shared_qualifications;
        let source_types = values
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        let scalar_type = terminal_scalar_type(primitive_type)?;
        let computation = crate::emission::structural_scalar_store_source::computation_root(
            checked,
            machine,
            state,
            statement_index,
            value,
            field_identity,
        )?;
        // The store may consume the SSA result of the scalar call this same
        // statement performed. That value has no authored local binding and so
        // no `AssignmentValue` expression row; it is reconstructed from the
        // call operation already lowered for this statement, which must be the
        // most recently established scalar value in this namespace.
        if let checked_trees::CheckedScalarStoreValue::ScalarResult { position } = value {
            let position = usize::try_from(*position).map_err(|_| {
                LoweringError::Unsupported("field store call-result position exceeds usize")
            })?;
            if position.checked_add(1) != Some(values.len()) {
                return unsupported(
                    "field store call result is not this statement's established scalar result",
                );
            }
            let value = values[position];
            return Ok(value);
        }
        if value.as_pure().is_some() {
            let expression = bindings.expression_at(
                identities,
                checked,
                state,
                statement_index,
                CheckedScalarExpressionRole::AssignmentValue,
            )?;
            if expression.scalar_type() != scalar_type {
                return unsupported("field store requires a matching pure value");
            }
            validate_direct_parameter_types(&expression, &scalar_carriers(&source_types))?;
            if !direct_expression_contains_short_circuit(&expression) {
                return Ok(ValueDeclaration {
                    qualifications: Default::default(),
                    id: emit_direct_expression(&expression, values, next_value, operations),
                    scalar_type,
                });
            }
            // A short-circuit Boolean has no single-block value: it evaluates
            // through its own decision blocks and rejoins with the result, the
            // staging an expanded computation's binding receives.
            let mut expansion = crate::scalar_lowering::computations::Expansion::new(
                checked,
                &qualifications,
                machine,
                1,
            )
            .with_structural_types(&self.structural_types)
            .enter_proof_scope(identities, &self.erased_proof_formals);
            let entry = expansion.direct_value(&source_types, 0, expression)?;
            let states = expansion.finish();
            return self.single_completed_expansion(
                identities,
                checked,
                &states,
                entry,
                scalar_type.into(),
                values,
                next_value,
                next_block,
                next_edge,
                operations,
                calls,
                "field assignment has no single completed RHS",
            );
        }
        let (root, role) = computation.ok_or(LoweringError::Unsupported(
            "field assignment lost its exact scalar computation root",
        ))?;
        // Complete the RHS with its full checked type. The field stores its
        // scalar payload after source checking has established its invariant;
        // that storage representation must not erase qualifications from the
        // evaluation edge or demand a bare type from an authored qualified cast.
        let result_type = crate::scalar_lowering::computations::computation_value_type(
            identities,
            checked,
            qualifications,
            root,
            &bindings,
            &source_types,
        )?;
        let mut expansion = self.scalar_computation_expansion(
            identities,
            checked,
            qualifications,
            machine,
            operations,
        )?;
        let entry = expansion.retained_value(
            identities,
            state,
            statement_index,
            role,
            &bindings,
            &source_types,
            result_type,
            0,
        )?;
        let states = expansion.finish();
        self.single_completed_expansion(
            identities,
            checked,
            &states,
            entry,
            result_type,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
            "field assignment has no single completed RHS",
        )
    }

    /// `complete_expansion` for a single declared result type, unwrapped to
    /// its one completed value; `message` names the caller's shape failure.
    #[allow(clippy::too_many_arguments)]
    fn single_completed_expansion(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        expansion: &crate::scalar_lowering::computations::PreparedExpansion,
        entry: usize,
        result_type: QualifiedScalarType,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
        message: &'static str,
    ) -> Result<ValueDeclaration, LoweringError> {
        let result = self.complete_expansion(
            identities,
            checked,
            expansion,
            entry,
            &[result_type],
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
        )?;
        match result.as_slice() {
            [value] => Ok(*value),
            _ => unsupported(message),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_expansion(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        checked: &CheckedTrees,
        expansion: &crate::scalar_lowering::computations::PreparedExpansion,
        entry_index: usize,
        result_types: &[QualifiedScalarType],
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<Vec<ValueDeclaration>, LoweringError> {
        // Scalar completion must not discard the storage identities produced
        // while evaluating its operands. Retirement uses the same namespace
        // as construction, without repeating the producer or treating reserved
        // but unevaluated slots as established values. These associations grant
        // no liveness: emitted control/ownership frontiers remain authoritative.
        crate::expression_preparation::bindings::retain_completed_structural_homes(
            &mut self.structural_locals,
            &expansion.storage,
        )?;
        let states = &expansion.states;
        if let Some(completed) = self.complete_straight_line(
            identities,
            checked,
            states,
            entry_index,
            result_types,
            values,
            next_value,
            operations,
            calls,
        )? {
            return Ok(completed);
        }
        let source_types = values
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        let mut completion_types = source_types.clone();
        completion_types.extend_from_slice(result_types);
        let completion_parameters = declarations(&completion_types, next_value)?;
        let completion = block_id(allocate_dense(next_block)?);
        let mut targets = vec![completion];
        let mut parameters = vec![completion_parameters.clone()];
        for state in states {
            targets.push(block_id(allocate_dense(next_block)?));
            parameters.push(declarations(&state.parameter_types, next_value)?);
        }
        self.blocks.push(Block {
            structural_parameters: std::mem::take(&mut self.block_structural_parameters),
            id: self.current,
            parameters: std::mem::take(&mut self.parameters),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: erased_proof_formal_declarations(
                identities,
                checked,
                &self.erased_proof_formals,
            ),
            operations: operations[self.operation_start..].to_vec(),
            terminator: Terminator::Jump {
                structural_arguments: Vec::new(),
                edge: edge_id(allocate_dense(next_edge)?),
                target: *targets.get(entry_index).ok_or(LoweringError::Unsupported(
                    "call computation entry is absent",
                ))?,
                arguments: values.iter().map(|value| value.id).collect(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: self
                    .proof_formal_forwarding()
                    .iter()
                    .map(|term| {
                        crate::scalar_lowering::contracts::lowered_proof_term(
                            checked,
                            term,
                            values,
                            &self.erased_scalar_formals,
                        )
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?,
                cleanup_actions: Vec::new(),
            },
        });
        for index in scalar_state_emission_order(states, entry_index)? {
            let state = &states[index];
            emit_state(
                identities,
                checked,
                state,
                targets[index + 1],
                &parameters[index + 1],
                &self.erased_scalar_formals,
                &self.erased_proof_formals,
                &targets,
                &parameters,
                self.machine,
                &self.shared_qualifications.catalog().sets,
                &mut self.qualification_coercions,
                next_value,
                next_block,
                next_edge,
                operations,
                calls,
                &mut self.blocks,
            )?;
        }
        operations.length_observations.clear();
        let result = completion_parameters[source_types.len()..].to_vec();
        *values = completion_parameters[..source_types.len()].to_vec();
        self.current = completion;
        self.parameters = completion_parameters;
        self.operation_start = operations.len();
        Ok(result)
    }
}

fn declarations(
    types: &[QualifiedScalarType],
    next_value: &mut u64,
) -> Result<Vec<ValueDeclaration>, LoweringError> {
    types
        .iter()
        .map(|scalar_type| {
            Ok(ValueDeclaration {
                qualifications: scalar_type.qualifications,
                id: value_id(allocate_dense(next_value)?),
                scalar_type: scalar_type.scalar_type,
            })
        })
        .collect()
}

pub(crate) fn validated_values(
    values: Option<&[ValueDeclaration]>,
    types: &[QualifiedScalarType],
) -> Result<Vec<ValueDeclaration>, LoweringError> {
    let values = values.ok_or(LoweringError::Unsupported(
        "call has no completed scalar operands",
    ))?;
    if values.len() != types.len()
        || values
            .iter()
            .zip(types)
            .any(|(value, expected)| value.value_type() != *expected)
    {
        return unsupported("completed call scalar operands disagree with the target signature");
    }
    Ok(values.to_vec())
}

/// Emit one finite-family `CallDynamic`'s runtime roster dispatch as ordinary
/// block control flow: the caller's block closes on the first tuple's binder
/// equality, each later binder test or tuple owns one guard block, and each
/// matched tuple performs the ordinary implementation-selector indirect call
/// on its own canonical slot before joining the merge block that following
/// operations continue in. `emit_leaf` pushes the leaf call operation for the
/// tuple at `index` and returns its completed value. Scalar and structural
/// results use the same ordinary edge transport into the merge parameter;
/// each leaf owns its result home until that transfer completes.
///
/// Keeping the row pick in the caller's own blocks leaves the emitted call
/// shape unchanged — each leaf names one exact interface slot through the
/// same `selector, slot` pair a static membership already admits — and keeps
/// the roster's source-produced tuple rows the only dispatch table this
/// mechanism consults.
/// One in-flight block's emission span: the identities and parameter
/// rosters a dispatch expansion closes on its guard terminator and reopens
/// at the merge block.
pub(crate) struct BlockEmissionSpan {
    pub(crate) erased_scalar_formals: Vec<ValueDeclaration>,
    pub(crate) erased_proof_formals: Vec<terminal_psi::ErasedProofFormal>,
    pub(crate) id: BlockId,
    pub(crate) parameters: Vec<ValueDeclaration>,
    pub(crate) structural_parameters: Vec<terminal_psi::StructuralParameterDeclaration>,
    pub(crate) operation_start: usize,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_dynamic_family_dispatch(
    span: &mut BlockEmissionSpan,
    blocks: &mut Vec<Block>,
    operations: &mut OperationBuffer,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    tuples: &[Vec<String>],
    family_actuals: &[ValueId],
    family_actual_types: &[ScalarType],
    merge_result: &OperationResult,
    mut emit_leaf: impl FnMut(
        &mut OperationBuffer,
        &mut u64,
        usize,
    ) -> Result<OperationResult, LoweringError>,
) -> Result<(), LoweringError> {
    let arity = family_actuals.len();
    if arity == 0 || arity != family_actual_types.len() {
        return unsupported("dynamic family call's actuals disagree with its binders");
    }
    if tuples.len() < 2 || tuples.iter().any(|tuple| tuple.len() != arity) {
        return unsupported("dynamic family roster does not cover the call's binders");
    }
    let merge = block_id(allocate_dense(next_block)?);
    let crash = block_id(allocate_dense(next_block)?);
    let next_block_id = |next_block: &mut u64| -> Result<BlockId, LoweringError> {
        Ok(block_id(allocate_dense(next_block)?))
    };
    let dispatch = (0..tuples.len())
        .map(|_| next_block_id(next_block))
        .collect::<Result<Vec<_>, _>>()?;
    let tests = tuples
        .iter()
        .map(|tuple| {
            (0..tuple.len())
                .map(|_| next_block_id(next_block))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let matched = emit_tuple_test(
        tuples,
        family_actuals,
        family_actual_types,
        next_value,
        operations,
        0,
        0,
    )?;
    let continue_target = |tuple: usize, binder: usize| -> BlockId {
        if binder + 1 < arity {
            tests[tuple][binder + 1]
        } else {
            dispatch[tuple]
        }
    };
    let next_tuple_target = |tuple: usize| -> BlockId {
        if tuple + 1 < tuples.len() {
            tests[tuple + 1][0]
        } else {
            crash
        }
    };
    blocks.push(Block {
        id: span.id,
        parameters: std::mem::take(&mut span.parameters),
        erased_scalar_formals: std::mem::take(&mut span.erased_scalar_formals),
        erased_proof_formals: std::mem::take(&mut span.erased_proof_formals),
        structural_parameters: std::mem::take(&mut span.structural_parameters),
        operations: operations[span.operation_start..].to_vec(),
        terminator: Terminator::Conditional {
            condition: matched,
            when_true: dispatch_successor_edge(next_edge, continue_target(0, 0))?,
            when_false: dispatch_successor_edge(next_edge, next_tuple_target(0))?,
        },
    });
    for (tuple_index, test_blocks) in tests.iter().enumerate() {
        for (binder, &test_block) in test_blocks
            .iter()
            .enumerate()
            .skip(usize::from(tuple_index == 0))
        {
            let start = operations.len();
            let matched = emit_tuple_test(
                tuples,
                family_actuals,
                family_actual_types,
                next_value,
                operations,
                tuple_index,
                binder,
            )?;
            blocks.push(Block {
                id: test_block,
                parameters: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                operations: operations[start..].to_vec(),
                terminator: Terminator::Conditional {
                    condition: matched,
                    when_true: dispatch_successor_edge(
                        next_edge,
                        continue_target(tuple_index, binder),
                    )?,
                    when_false: dispatch_successor_edge(next_edge, next_tuple_target(tuple_index))?,
                },
            });
        }
    }
    for (tuple_index, block) in dispatch.iter().enumerate() {
        let start = operations.len();
        let produced = emit_leaf(operations, next_value, tuple_index)?;
        blocks.push(Block {
            id: *block,
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: operations[start..].to_vec(),
            terminator: Terminator::Jump {
                edge: edge_id(allocate_dense(next_edge)?),
                target: merge,
                arguments: produced
                    .as_scalar()
                    .map(|declaration| declaration.id)
                    .into_iter()
                    .collect(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: produced
                    .structural()
                    .map(|result| terminal_psi::StructuralArgument {
                        place: result.place,
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    })
                    .into_iter()
                    .collect(),
                cleanup_actions: Vec::new(),
            },
        });
    }
    blocks.push(Block {
        id: crash,
        parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Crash {
            edge: edge_id(allocate_dense(next_edge)?),
            cause: terminal_psi::CrashCause::Abort,
            site_guard: Vec::new(),
            frontier_lower_bound: Vec::new(),
        },
    });
    span.id = merge;
    span.parameters = merge_result.as_scalar().into_iter().collect();
    span.structural_parameters = merge_result
        .structural()
        .map(|result| StructuralParameterDeclaration {
            place: result.place,
            position: 0,
            structural_type: result.structural_type,
            multiplicity: result.multiplicity,
            access: StructuralAccess::Owned,
            qualifications: result.qualifications.clone(),
            projected_qualifications: result.projected_qualifications.clone(),
        })
        .into_iter()
        .collect();
    span.operation_start = operations.len();
    Ok(())
}

/// Emit the constant and equality test matching tuple `tuple_index`'s binder
/// `binder` against the call's evaluated actual, inside the caller's current
/// operation segment.
fn emit_tuple_test(
    tuples: &[Vec<String>],
    family_actuals: &[ValueId],
    family_actual_types: &[ScalarType],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
    tuple_index: usize,
    binder: usize,
) -> Result<ValueId, LoweringError> {
    let carrier = family_actual_types[binder]
        .as_integer()
        .ok_or(LoweringError::Unsupported(
            "dynamic family actual has no integer carrier",
        ))?;
    let value = family_tuple_integer(&tuples[tuple_index][binder], carrier.sign())?;
    let constant = crate::emission::operation_emission::expressions::emit_scalar_leaf(
        OperationKind::IntegerConstant { value },
        family_actual_types[binder],
        next_value,
        operations,
    );
    Ok(
        crate::emission::operation_emission::expressions::emit_scalar_leaf(
            OperationKind::IntegerEqual {
                left: family_actuals[binder],
                right: constant,
            },
            ScalarType::Boolean,
            next_value,
            operations,
        ),
    )
}

/// Decode one canonical family tuple member (`named(integer-const(v))`) into
/// the integer constant the binder's signedness carries.
fn family_tuple_integer(
    identity: &str,
    sign: semantic_vocabulary::IntegerSign,
) -> Result<semantic_vocabulary::IntegerValue, LoweringError> {
    let value = identity
        .strip_prefix("named(integer-const(")
        .and_then(|inner| inner.strip_suffix("))"))
        .and_then(|digits| digits.parse::<i128>().ok())
        .ok_or(LoweringError::Unsupported(
            "dynamic family tuple identity is not an integer constant",
        ))?;
    semantic_vocabulary::IntegerValue::for_sign(sign, value).map_err(|_| {
        LoweringError::Unsupported("dynamic family tuple overflows its binder carrier")
    })
}

fn dispatch_successor_edge(
    next_edge: &mut u64,
    target: BlockId,
) -> Result<terminal_psi::SuccessorEdge, LoweringError> {
    Ok(terminal_psi::SuccessorEdge {
        edge: edge_id(allocate_dense(next_edge)?),
        target,
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        cleanup_actions: Vec::new(),
    })
}
