//! Scalar operand evaluation within an existing machine's structural frontier.
use super::super::{
    BlockId, QualifiedScalarType, StructuralAccess, StructuralArgument,
    StructuralParameterDeclaration, StructuralTypeDeclaration, block_id, scalar_carriers,
};
use super::{
    Block, CheckedScalarExpressionRole, CheckedTrees, CheckedUnitEffectOperationPlan,
    LoweringError, Multiplicity, Operation, OperationResult, PlaceId, ScalarType,
    StructuralMultiplicity, StructuralTypeShape, Terminator, ValueDeclaration, allocate_dense,
    direct_expression_contains_short_circuit, edge_id, emit_direct_expression,
    terminal_scalar_type, unsupported, validate_direct_parameter_types, value_id,
};
use crate::direct_calls::{DirectCallRole, call_structural_arguments_mut, direct_call_parts};
use crate::emission::borrowed_window::terminal_multiplicity;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::buffer::{OperationBuffer, StructuralBinding};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::scalar_graph::scalar_contracts::erased_proof_formal_declarations;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::{
    LoweredScalarBranchState, ScalarStateTargets, scalar_state_emission_order,
};
use checked_trees::CheckedCallScalarArgument;
use state_emission::emit_state;

mod record_captures;
mod scalar_control;
mod source_values;
mod state_emission;

pub(crate) use scalar_control::ScalarReturnCleanup;

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
    pub(crate) subslices: Vec<crate::scalar_graph::scalar_computations::subslices::Slot>,
    pub(crate) produced_values:
        Vec<crate::scalar_graph::scalar_computations::produced_values::Slot>,
    pub(crate) literals: Vec<crate::scalar_graph::scalar_computations::literals::Slot>,
    pub(crate) copies: Vec<crate::scalar_graph::scalar_computations::copies::Slot>,
    pub(crate) cases: Vec<crate::scalar_graph::scalar_computations::cases::Slot>,
    pub(crate) record_fields: Vec<crate::scalar_graph::scalar_computations::fields::Binding>,
    pub(crate) place_observations: std::sync::Arc<
        [crate::expression_preparation::bindings::place_observations::PlaceObservation],
    >,
    pub primitive_storage: Vec<(symbols::SymbolHandle, PlaceId, ScalarType)>,
    /// State-local storage has its own namespace; it is not an immutable slot.
    /// Other callers retain the ordinary dense source-prefix mapping.
    pub scalar_bindings: Option<crate::expression_preparation::bindings::ScalarBindings>,
    pub structural_parameters: Vec<(u32, StructuralParameterDeclaration)>,
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

/// Replace candidate roots only at their normal death edge. Both rosters are
/// in reverse establishment order; physical source declarations stay intact.
/// An ordinary move has no residual candidates and only rebinds survivors.
#[derive(Clone)]
pub(crate) struct SelectionCleanup {
    pub(crate) selected: PlaceId,
    pub(crate) sources: Vec<PlaceId>,
    pub(crate) remaining: Vec<PlaceId>,
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
    /// Resolve one selected death in the live state value namespace. Result
    /// registries differ between ordinary and graph bodies; owner/parameter
    /// lookup and selection rebinding do not. The caller supplies its registry.
    pub(crate) fn cleanup_storage(
        &self,
        source: &checked_trees::CheckedUnitStructuralArgumentSourcePlan,
        parameters: &[StructuralParameterDeclaration],
        mut result: impl FnMut(
            u32,
        ) -> Result<
            (PlaceId, semantic_vocabulary::StructuralTypeId),
            LoweringError,
        >,
    ) -> Result<(PlaceId, semantic_vocabulary::StructuralTypeId), LoweringError> {
        use checked_trees::CheckedUnitStructuralArgumentSourcePlan;
        let (place, structural_type) = match *source {
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                let parameter = parameters
                    .get(parameter_index as usize)
                    .ok_or(LoweringError::Unsupported("cleanup parameter is absent"))?;
                (parameter.place, parameter.structural_type)
            }
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                let local = self
                    .structural_locals
                    .iter()
                    .find(|local| local.root == facts::PlaceRoot::Symbol(symbol))
                    .ok_or(LoweringError::Unsupported(
                        "cleanup local is not established",
                    ))?;
                if !local.argument.path.is_empty() {
                    return unsupported("cleanup local is not its whole owner");
                }
                (local.argument.place, local.structural_type)
            }
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                result(binding_ordinal)?
            }
            _ => return unsupported("cleanup source has no owned structural storage"),
        };
        Ok((self.current_structural_place(place), structural_type))
    }

    /// Bind the emitted structural signature before evaluating any operands.
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
            cases: self.cases.clone(),
            record_fields: self.record_fields.clone(),
            place_observations: self.place_observations.clone(),
            primitive_storage: self.primitive_storage.clone(),
            scalar_bindings: self.scalar_bindings.clone(),
            structural_parameters: self.structural_parameters.clone(),
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

    /// Publish one completed structural result into the state's existing value
    /// and local namespaces. Calls and constructors share the original home;
    /// registration neither copies payload fields nor changes cleanup custody.
    /// The validated producer identifies whether this is the statement result:
    /// nested operands share its statement coordinate but own no source local.
    pub(crate) fn establish_structural_result(
        &mut self,
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        is_statement_result: bool,
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
            // Registration does not establish new linear authority. Only the
            // exact ordinary call result already emitted under replayed custody
            // can attach that returned frontier to a local binding.
            let mut producers = operations.iter().filter(|operation| {
                matches!(&operation.result,
                OperationResult::Structural(value) if value == &produced)
                    && direct_call_parts(&operation.kind)
                        .is_some_and(|call| call.role == DirectCallRole::Structural)
            });
            let producer = producers.next().ok_or(LoweringError::Unsupported(
                "linear local has no ordinary call result",
            ))?;
            if producers.next().is_some()
                || produced.claims.is_empty()
                || !operations.source_calls.iter().any(|occurrence| {
                    occurrence.source_state == state
                        && occurrence.statement_index == result.statement_index as usize
                        && occurrence.terminal_operation == producer.id
                })
            {
                return unsupported("linear local lost its exact call result custody");
            }
        }
        self.establish_structural_binding(
            checked,
            state,
            result,
            is_statement_result,
            produced.into(),
            structural_types,
            operations,
        )
    }

    /// Bind an already established place to its authored local. Producer-specific
    /// admission happens before this shared namespace and local-symbol step.
    pub(crate) fn establish_structural_binding(
        &mut self,
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        is_statement_result: bool,
        produced: StructuralBinding,
        structural_types: &[StructuralTypeDeclaration],
        operations: &mut OperationBuffer,
    ) -> Result<(), LoweringError> {
        let multiplicity = produced.multiplicity();
        let declaration = structural_types
            .iter()
            .find(|declaration| declaration.id == produced.structural_type())
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
        let (machine, source) = crate::declarations::state_declaration(checked, state)?;
        let local = match checked
            .statement_table
            .statements(source.statement_nodes)
            .get(result.statement_index as usize)
        {
            Some(checked_trees::statement::StatementNode::LocalData(local))
                if is_statement_result =>
            {
                Some(local)
            }
            Some(_) => None,
            None => return unsupported("structural result lost its source statement"),
        };
        let mut local_symbol = symbols::SymbolHandle::invalid();
        if let Some(local) = local {
            // The element-declared view spelling (`let v: u8 = coll[a..b]`)
            // binds the subslice's own result type; a declared view keeps its
            // spelling. Custody and carrier resolve on the bound type.
            let binding_type =
                crate::expression_preparation::source_custody::structural::declared_view_binding_type(
                    checked,
                    machine,
                    source,
                    local.type_reference,
                    local.initial_value,
                );
            // A `&T` local holds shared-borrow custody of the referent's
            // storage; its carrier identity is the record `T`, matching the
            // structural type `add_type` recorded for the result. A `&mut`
            // view local joins the same referent carrier — the minted binding
            // is affine loan custody even though the borrow shell's own type
            // multiplicity spells Unrestricted.
            let shared_borrow =
                crate::expression_preparation::source_custody::structural::shared_borrow_record_referent(
                    checked,
                    binding_type,
                );
            let mutable_borrow =
                crate::reference_custody::mutable_borrow_parts(checked, binding_type);
            // A `&mut` slice view's carrier is its element-view carrier
            // exactly as a `&` view's is: `let slots: &mut [Cell] = ..` binds
            // the `[Cell]` view — and `&mut Cell` binds the `Cell` record —
            // the same peel `add_type` applied recording the result identity.
            // A call result that returns a record `&T` keeps its `&T` shell:
            // the binding stamps the callee's `ref(named(T))` return
            // identity, so only a borrow-carrier local peels to its referee.
            // A `&[T]` view or `&u8` call result still peels — its binding
            // stamps the view or primitive carrier itself.
            let reference_carrier =
                matches!(declaration.shape, StructuralTypeShape::Reference { .. });
            let record_call_result = shared_borrow.is_some_and(|referent| {
                matches!(
                    checked.expression_table.expression(local.initial_value),
                    checked_trees::expression::ExpressionNode::Call(_)
                ) && matches!(
                    checked.type_reference_table.type_reference(referent),
                    checked_trees::types::TypeReferenceNode::Named { .. }
                )
            });
            let carrier = match shared_borrow
                .filter(|_| !record_call_result && !reference_carrier)
                .or(
                    crate::expression_preparation::source_custody::structural::mutable_view_carrier(
                        checked,
                        binding_type,
                    ),
                )
                .or(
                    crate::expression_preparation::source_custody::structural::shared_slice_view_referent(
                        checked,
                        binding_type,
                    ),
                ) {
                Some(referent) => super::parameters::structural_carrier_type(checked, referent)?,
                None => super::parameters::structural_carrier_type(checked, binding_type)?,
            };
            // A materialized Reference shape always carries affine custody.
            // Other stored values use the shared recursive custody judgment,
            // including arrays of references. Source copyability alone cannot
            // authorize duplicating those captured loan descriptors.
            let expected_multiplicity = if reference_carrier {
                Multiplicity::Affine
            } else {
                crate::reference_custody::binding_multiplicity(checked, binding_type)
            };
            if !local.symbol.is_valid()
                || checked.normalized_type_identity(carrier).as_str() != result.type_identity
                || expected_multiplicity != result.multiplicity
                || self
                    .structural_locals
                    .iter()
                    .any(|binding| binding.root == facts::PlaceRoot::Symbol(local.symbol))
            {
                return unsupported("structural value repeats or substitutes its local binding");
            }
            local_symbol = local.symbol;
            self.structural_fields
                .adopt_declarations(produced.structural_type(), structural_types);
            self.structural_locals.push(
                crate::expression_preparation::bindings::StructuralLocalBinding {
                    result_binding_ordinal: Some(result.binding_ordinal),
                    root: facts::PlaceRoot::Symbol(local.symbol),
                    structural_type: produced.structural_type(),
                    argument: StructuralArgument {
                        place: produced.place(),
                        path: produced.path().to_vec(),
                        // A reference home owns its descriptor. Borrow access belongs
                        // to the explicit Referent crossing, not that home.
                        access: if reference_carrier {
                            StructuralAccess::Owned
                        } else if shared_borrow.is_some() {
                            StructuralAccess::SharedBorrow
                        } else if mutable_borrow.is_some() {
                            StructuralAccess::MutableBorrow
                        } else {
                            StructuralAccess::Owned
                        },
                    },
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
                        access: if mutable_borrow.is_some() {
                            StructuralAccess::MutableBorrow
                        } else if shared_borrow.is_some() {
                            StructuralAccess::SharedBorrow
                        } else {
                            StructuralAccess::Owned
                        },
                    },
                );
            }
        }
        // Residual cleanup tracks values this statement established. A
        // projected owned alias established none: the root it names keeps its
        // own owner row and its own residual complement.
        if multiplicity == StructuralMultiplicity::Affine && produced.path().is_empty() {
            self.structural_value_owners.push(StructuralValueOwner {
                symbol: local_symbol,
                statement: result.statement_index,
                value: produced.operation_result()?.clone(),
            });
        }
        operations
            .structural_values
            .push((result.binding_ordinal, produced));
        Ok(())
    }

    /// Call operand resolvers retain authored result identities. Apply only
    /// transports that precede the operation; earlier calls keep their places.
    pub(crate) fn remap_transported_call_operands(&mut self, operations: &mut OperationBuffer) {
        let cleanups = &self.selection_cleanups;
        let remap = |operation: &mut Operation| {
            let Some(arguments) = call_structural_arguments_mut(&mut operation.kind) else {
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

    pub(crate) fn selection_return_discards(
        &self,
        mut roots: Vec<(PlaceId, bool)>,
    ) -> Result<Vec<PlaceId>, LoweringError> {
        for cleanup in &self.selection_cleanups {
            let selected = roots
                .iter()
                .position(|(place, _)| *place == cleanup.selected)
                .ok_or(LoweringError::Unsupported(
                    "selected cleanup result is absent from its operation roster",
                ))?;
            if cleanup.sources.is_empty() {
                if !cleanup.remaining.is_empty() {
                    return unsupported("owned move cannot fabricate residual cleanup roots");
                }
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
                || !roots[start..end]
                    .iter()
                    .filter(|(place, _)| cleanup.sources.contains(place))
                    .map(|(place, _)| place)
                    .eq(cleanup.sources.iter())
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
            // Candidates interleaved with other live owners share the joined
            // frontier: a residual parameter stands at each surviving
            // candidate's roster position in reverse order, the displaced
            // candidate's row drops with the result parameter that displaced
            // its slot, and transported owners keep their rebound parameter
            // places at their own roster positions. Owners established
            // between candidates therefore stay live in establishment order
            // rather than being flattened into or out of the source run.
            let mut remaining = cleanup.remaining.iter();
            let mut source_rows = 0usize;
            let replacement = roots[start..end]
                .iter()
                .filter_map(|(place, discard)| {
                    if cleanup.sources.contains(place) {
                        source_rows += 1;
                        if source_rows == cleanup.sources.len() {
                            return None;
                        }
                        return remaining.next().map(|place| (*place, true));
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
            if remaining.next().is_some() {
                return unsupported(
                    "owned selection cleanup residuals do not cover their source roster",
                );
            }
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
        mut cleanup_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &OperationBuffer,
    ) -> Result<semantic_vocabulary::EdgeId, LoweringError> {
        for action in &mut cleanup_actions {
            let place = match action {
                terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) => place,
                terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                    &mut discard.place
                }
                terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                    &mut cleanup.place
                }
            };
            *place = self.current_structural_place(*place);
        }
        let types = values
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        let parameters = declarations(&types, next_value)?;
        let continuation = block_id(allocate_dense(next_block)?);
        let edge = edge_id(allocate_dense(next_edge)?);
        self.blocks.push(Block {
            structural_parameters: std::mem::take(&mut self.block_structural_parameters),
            id: self.current,
            parameters: std::mem::take(&mut self.parameters),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: erased_proof_formal_declarations(&self.erased_proof_formals),
            operations: operations[self.operation_start..].to_vec(),
            terminator: Terminator::Jump {
                structural_arguments: Vec::new(),
                edge,
                target: continuation,
                arguments: values.iter().map(|value| value.id).collect(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: self
                    .proof_formal_forwarding()
                    .iter()
                    .map(|term| {
                        crate::scalar_graph::scalar_contracts::lowered_proof_term(
                            term,
                            values,
                            &self.erased_scalar_formals,
                        )
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?,
                cleanup_actions,
            },
        });
        *values = parameters.clone();
        self.parameters = parameters;
        self.current = continuation;
        self.operation_start = operations.len();
        Ok(edge)
    }

    /// A local refinement is proved where its initializer has completed. The
    /// ordinary continuation carries the exact evaluated values; its arrival
    /// obligation precedes every use of the newly established local.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn require_value_refinements(
        &mut self,
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
        let predicate = crate::scalar_graph::scalar_contracts::clauses(
            &clauses,
            values,
            &self.erased_scalar_formals,
            &crate::scalar_graph::scalar_contracts::ContractViewNamespace::EMPTY,
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
            cases: Vec::new(),
            record_fields: Vec::new(),
            place_observations: std::sync::Arc::from([]),
            primitive_storage: Vec::new(),
            scalar_bindings: None,
            structural_fields: Default::default(),
            structural_cases: Vec::new(),
            structural_parameters: Vec::new(),
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
    fn proof_formal_forwarding(
        &self,
    ) -> Vec<crate::scalar_graph::scalar_contracts::LoweredProofTerm> {
        (0..self.erased_proof_formals.len())
            .map(
                |position| crate::scalar_graph::scalar_contracts::LoweredProofTerm::Formal {
                    position: u32::try_from(position)
                        .expect("erased-proof roster positions fit u32"),
                },
            )
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn arguments(
        &mut self,
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

    /// Complete one dense scalar operand at its original call coordinate.
    /// The caller retains the result in `values` before evaluating another
    /// operand and uses indices to follow values across private blocks. Only
    /// the pre-group source prefix may be read by authored operand expressions;
    /// later retained slots carry private argument temporaries across the CFG.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn argument_at(
        &mut self,
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
        let source_bindings = self
            .scalar_bindings
            .clone()
            .unwrap_or_else(|| {
                crate::expression_preparation::bindings::ScalarBindings::new(
                    values
                        .iter()
                        .take(source_value_count)
                        .map(|value| value.scalar_type),
                )
            })
            .with_structural_parameters(&self.structural_parameters)
            .with_resolved_structural_observations(&self.structural_fields, &self.structural_cases)
            .with_place_observations(&self.place_observations);
        let source_bindings = source_bindings
            .with_primitive_storage(&self.primitive_storage)
            .with_structural_locals(&self.structural_locals)
            .with_view_locals(&self.view_locals)
            .with_element_views(&self.element_views);
        let Some(call) = operation.call() else {
            return Ok(None);
        };
        // A selector call's selector is its first operand, ahead of the data
        // arguments. Staged per-ordinal evaluation numbers the data arguments
        // only, so it does not yet carry a selector.
        let operands = call
            .scalar_operands()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        if argument_ordinal.is_some() && call.dynamic_selector().is_some() {
            return unsupported("a staged argument group cannot yet carry a selector call");
        }
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
                    .expression(&checked.facts.proven_scalar_indexes, expression)?
                    .value_type(&source_types),
                CheckedCallScalarArgument::Computation(root) => {
                    let nodes = &checked.facts.values.scalar_computations.nodes;
                    if !nodes.is_valid(*root) {
                        return unsupported("call argument computation has no live root");
                    }
                    crate::scalar_graph::scalar_computations::computation_value_type(
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
                .expression(&checked.facts.proven_scalar_indexes, expression)
                .is_ok_and(|expression| direct_expression_contains_short_circuit(&expression)),
        });
        if !needs_control {
            return arguments
                .iter()
                .zip(&argument_types)
                .map(|(argument, scalar_type)| {
                    let expression = source_bindings.expression(
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

        let mut expansion = crate::scalar_graph::scalar_computations::Expansion::new(
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
        .with_cases(&self.cases)
        .enter_proof_scope(&self.erased_proof_formals)
        .with_fields(&self.record_fields);
        let entry_index = expansion.call_arguments(
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
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement_index: u32,
        value: &checked_trees::CheckedStoreValue,
        primitive_type: typed_trees::types::PrimitiveType,
        field_identity: Option<&str>,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        let bindings = self
            .scalar_bindings
            .clone()
            .unwrap_or_else(|| {
                crate::expression_preparation::bindings::ScalarBindings::new(
                    values.iter().map(|value| value.scalar_type),
                )
            })
            .with_primitive_storage(&self.primitive_storage)
            .with_view_locals(&self.view_locals)
            .with_element_views(&self.element_views)
            .with_structural_parameters(&self.structural_parameters)
            .with_structural_locals(&self.structural_locals)
            .with_resolved_structural_observations(&self.structural_fields, &self.structural_cases)
            .with_place_observations(&self.place_observations);
        let qualifications = &self.shared_qualifications;
        let source_types = values
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        let scalar_type = terminal_scalar_type(primitive_type)?;
        let computation_role = crate::emission::structural_scalar_store_source::computation_root(
            checked,
            machine,
            state,
            statement_index,
            value,
            field_identity,
        )?
        .map(|(_, role)| role);
        // The store may consume the SSA result of the scalar call this same
        // statement performed. That value has no authored local binding and so
        // no `AssignmentValue` expression row; it is reconstructed from the
        // call operation already lowered for this statement, which must be the
        // most recently established scalar value in this namespace.
        if let checked_trees::CheckedStoreValue::ScalarResult { position } = value {
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
            let mut expansion = crate::scalar_graph::scalar_computations::Expansion::new(
                checked,
                &qualifications,
                machine,
                1,
            )
            .with_structural_types(&self.structural_types)
            .enter_proof_scope(&self.erased_proof_formals);
            let entry = expansion.direct_value(&source_types, 0, expression)?;
            let states = expansion.finish();
            let result = self.complete_expansion(
                &states,
                entry,
                &[scalar_type.into()],
                values,
                next_value,
                next_block,
                next_edge,
                operations,
                calls,
            )?;
            return match result.as_slice() {
                [value] => Ok(*value),
                _ => unsupported("field assignment has no single completed RHS"),
            };
        }
        let mut expansion = crate::scalar_graph::scalar_computations::Expansion::new(
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
        .with_cases(&self.cases)
        .enter_proof_scope(&self.erased_proof_formals)
        .with_fields(&self.record_fields);
        let entry = expansion.retained_value(
            state,
            statement_index,
            computation_role.unwrap_or(CheckedScalarExpressionRole::AssignmentValue),
            &bindings,
            &source_types,
            scalar_type.into(),
            0,
        )?;
        let states = expansion.finish();
        let result = self.complete_expansion(
            &states,
            entry,
            &[scalar_type.into()],
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
        )?;
        match result.as_slice() {
            [value] => Ok(*value),
            _ => unsupported("field assignment has no single completed RHS"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_expansion(
        &mut self,
        states: &[LoweredScalarBranchState],
        entry_index: usize,
        result_types: &[QualifiedScalarType],
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<Vec<ValueDeclaration>, LoweringError> {
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
            erased_proof_formals: erased_proof_formal_declarations(&self.erased_proof_formals),
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
                        crate::scalar_graph::scalar_contracts::lowered_proof_term(
                            term,
                            values,
                            &self.erased_scalar_formals,
                        )
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?,
                cleanup_actions: Vec::new(),
            },
        });
        for index in scalar_state_emission_order(
            states,
            entry_index,
            ScalarStateTargets::CompletionThenStates,
        )? {
            let state = &states[index];
            emit_state(
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
        operations.byte_lengths.clear();
        operations.element_lengths.clear();
        operations.field_byte_lengths.clear();
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
