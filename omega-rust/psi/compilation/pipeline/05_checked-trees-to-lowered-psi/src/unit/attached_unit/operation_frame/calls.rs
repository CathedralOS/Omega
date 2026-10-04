//! Calls into the closure's own bodies: internal Unit and structural calls,
//! the member calls a structural-value construction makes, claim-free affine
//! leaf calls and scalar calls. `emit_call` is the one entry both routes use;
//! boundary calls continue in `boundary_calls`.
//!
//! Every call publishes its result through the frame's representations:
//! a scalar result through `bind_scalar_result` (the dense value namespace
//! plus a composed state's binding namespace), and a structural result
//! through `publish_result` (the dense roster with its local binding, or the
//! composed catalog and operation-buffer registry).

use super::super::parameters::{
    self, StructuralResultCustody, emitted_claim_transfers, lower_structural_arguments,
    validate_transfer_shape,
};
use super::super::{
    argument_evaluation, bodies::UnitBody, ordinary_calls, signatures, structural_calls,
    structural_values,
};
use super::{OperationFrame, StructuralResults};
use crate::direct_calls::{DirectCall, DirectCallResult, ScalarCallFrame};
use crate::emission::operation_emission::buffer::{OperationBuffer, SourceCallCoordinate};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::scalar_graph::scalar_call_closure::callee::{CheckedScalarCallee, PreparedScalarCallee};
use crate::unit::{
    CheckedUnitEffectOperationPlan, LoweringError, Multiplicity, Operation, OperationResult,
    PlaceId, StructuralPlaceDeclaration, StructuralPlaceKind, ValueDeclaration, allocate_dense,
    lookup_machine_id, lower_checked_crash_route_buckets, terminal_scalar_type, unsupported,
    value_id,
};
use checked_trees::{CheckedUnitCallResult, CheckedUnitCallRoute};
use semantic_vocabulary::ScalarQualificationSetId;

/// The completed operands one call hands the frame. Each route evaluates
/// them on its own schedule before the call.
pub(in crate::unit::attached_unit) struct CallInputs<'i> {
    /// Completed scalar operands in the callee's retained operand order.
    /// Only a selected operator, which evaluates its own direct operands,
    /// arrives without them.
    pub(in crate::unit::attached_unit) evaluated_scalar_arguments: Option<&'i [ValueDeclaration]>,
    /// The places of the call's byte-sequence literal and subslice
    /// arguments, in argument order.
    pub(in crate::unit::attached_unit) byte_places: &'i [PlaceId],
    /// A staged call completes a nested argument group: its scalar result is
    /// a private temporary the route retains itself, never a source binding.
    pub(in crate::unit::attached_unit) staged: bool,
}

impl OperationFrame<'_, '_> {
    /// Emit one call. Returns the scalar result of a staged scalar call for
    /// the route to retain; every other result is already published.
    pub(in crate::unit::attached_unit) fn emit_call(
        mut self,
        operation: &CheckedUnitEffectOperationPlan,
        inputs: CallInputs<'_>,
    ) -> Result<Option<ValueDeclaration>, LoweringError> {
        let Some(call) = operation.call() else {
            return unsupported("operation frame call entry received a non-call operation");
        };
        match (call.route(), call.result()) {
            // A structural call whose target is not a Unit body is a legacy
            // claim-free affine leaf with its own checked return plan.
            (CheckedUnitCallRoute::Module { .. }, CheckedUnitCallResult::Structural { .. })
                if !UnitBody::contains(self.callees.plans, call.target.machine) =>
            {
                self.claim_free_structural_call(operation, &inputs)?;
                Ok(None)
            }
            (
                CheckedUnitCallRoute::Module { .. },
                CheckedUnitCallResult::Unit | CheckedUnitCallResult::Structural { .. },
            ) => {
                self.unit_call(operation, &inputs)?;
                Ok(None)
            }
            (CheckedUnitCallRoute::Module { .. }, CheckedUnitCallResult::Scalar(_)) => {
                let value = self.scalar_call(operation, &inputs)?;
                self.bind_scalar_result(call.coordinate, value, inputs.staged)
            }
            (CheckedUnitCallRoute::Boundary, CheckedUnitCallResult::Unit) => {
                self.boundary_call(operation, &inputs)?;
                Ok(None)
            }
            (CheckedUnitCallRoute::Boundary, CheckedUnitCallResult::Scalar(_)) => {
                let value = self.boundary_scalar_call(operation, &inputs)?;
                self.bind_scalar_result(call.coordinate, value, inputs.staged)
            }
            (CheckedUnitCallRoute::Boundary, CheckedUnitCallResult::Structural { .. }) => {
                self.boundary_structural_call(operation, &inputs)?;
                Ok(None)
            }
        }
    }

    /// A retained scalar call result is the next dense value, so it also
    /// enters a composed state's binding namespace at its checked ordinal,
    /// exactly like an established scalar local. Only a result that
    /// initializes a declared immutable local also takes an authored slot;
    /// one an assignment stores stays unnamed, or every later local's
    /// authored read would resolve to it. A discarded result keeps its typed
    /// operation result without occupying a source position; a staged result
    /// goes back to the route.
    fn bind_scalar_result(
        &mut self,
        coordinate: checked_trees::CheckedUnitCallCoordinate,
        value: ValueDeclaration,
        staged: bool,
    ) -> Result<Option<ValueDeclaration>, LoweringError> {
        let names_local =
            crate::emission::call_source_custody::initializers::initializes_immutable_local(
                self.checked,
                self.state,
                coordinate,
            )?;
        let value = if names_local {
            self.refine_scalar_local_value(coordinate.statement_index, value)?
        } else {
            value
        };
        if staged {
            return Ok(Some(value));
        }
        if !crate::emission::call_source_custody::initializers::discards_result(
            self.checked,
            self.state,
            coordinate,
        )? {
            let position = self.values.len();
            self.values.push(value);
            if let Some(bindings) = self.evaluation.scalar_bindings.as_mut() {
                if names_local {
                    bindings.append(
                        checked_trees::CheckedScalarBindingDestination::Immutable,
                        value.scalar_type,
                        position,
                    )?;
                } else {
                    bindings.append_unnamed_result(position);
                }
            }
        }
        Ok(None)
    }

    /// Publish one completed structural call result. The dense roster keeps
    /// the row with its return-discard custody and binds a closed-array
    /// local; a composed state adds the place to its catalog and registers
    /// the value, with its source local, in the operation buffer. The dense
    /// route registers the value itself when the call emits it.
    pub(super) fn publish_result(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        declaration: StructuralPlaceDeclaration,
        discard_on_return: bool,
    ) -> Result<(), LoweringError> {
        self.results.push(declaration, discard_on_return);
        let StructuralPlaceKind::OperationResult { producer, .. } = declaration.kind else {
            return unsupported("structural call result has no producing operation");
        };
        let produced = self
            .operations
            .iter()
            .find(|operation| operation.id == producer)
            .and_then(|operation| operation.result.structural())
            .cloned()
            .ok_or(LoweringError::Unsupported(
                "structural call did not establish its result",
            ))?;
        if matches!(self.results, StructuralResults::Dense(_)) {
            structural_values::bind_local(
                self.checked,
                self.state,
                operation,
                declaration,
                &mut self.evaluation.structural_locals,
            )?;
            // A view local this call bound joins the view roster: its lengths
            // and element reads resolve through the place the locals join
            // named, and the produced type names which view family it is.
            if let Some(carrier) =
                crate::expression_preparation::bindings::view_locals::ViewCarrier::of(
                    produced.structural_type,
                    self.structural_types.declarations(),
                )
            {
                let (_, source) = crate::expression_preparation::source_custody::authored_state(
                    self.checked,
                    self.state,
                )?;
                if let Some(checked_trees::statement::StatementNode::LocalData(local)) = self
                    .checked
                    .statement_table
                    .statements(source.statement_nodes)
                    .get(result.statement_index as usize)
                    && local.symbol.is_valid()
                    && self
                        .evaluation
                        .structural_locals
                        .iter()
                        .any(|binding| binding.root == facts::PlaceRoot::Symbol(local.symbol))
                    && !self
                        .evaluation
                        .view_locals
                        .iter()
                        .any(|binding| binding.symbol == local.symbol)
                {
                    let access = if crate::expression_preparation::source_custody::structural::mutable_view_carrier(
                        self.checked,
                        local.type_reference,
                    )
                    .is_some()
                    {
                        terminal_psi::StructuralAccess::MutableBorrow
                    } else if crate::expression_preparation::source_custody::structural::shared_slice_view_referent(
                        self.checked,
                        local.type_reference,
                    )
                    .is_some()
                    {
                        terminal_psi::StructuralAccess::SharedBorrow
                    } else {
                        terminal_psi::StructuralAccess::Owned
                    };
                    self.evaluation.view_locals.push(
                        crate::expression_preparation::bindings::view_locals::ViewLocalBinding {
                            symbol: local.symbol,
                            place: declaration.id,
                            structural_type: produced.structural_type,
                            carrier,
                            access,
                        },
                    );
                }
            }
            return Ok(());
        }
        // Any call with a structural result, ordinary or boundary.
        let Some(coordinate) = operation
            .call()
            .filter(|call| call.structural_result().is_some())
            .map(|call| call.coordinate)
        else {
            return unsupported("structural result has no call coordinate");
        };
        let authored = crate::emission::call_source_custody::authored::locate_source(
            self.checked,
            self.state,
            coordinate,
        )?;
        let (_, source) = crate::expression_preparation::source_custody::authored_state(
            self.checked,
            self.state,
        )?;
        // A nested call can have ordinal zero when the initializer itself is
        // a constructor. Only the exact initializer occurrence owns the local.
        let is_statement_result = matches!(
            (self.checked.statement_table.statements(source.statement_nodes)
                .get(result.statement_index as usize), authored.source_site),
            (Some(checked_trees::statement::StatementNode::LocalData(local)),
             Some(checked_trees::NominalMachineUseSite::Expression(expression)))
                if local.initial_value == expression
        );
        // A computed dispatch owns one anonymous result. Publish its exact
        // source identity after ordinary result registration; copied payload
        // receivers then read this home without running their call again.
        // A nested operand or the selected terminal call shares the statement
        // but is not the canonical subject producer.
        let saved_subject = match authored.source_site {
            Some(checked_trees::NominalMachineUseSite::Expression(expression)) => {
                validation::saved_dispatch_subject(
                    &self.checked.typed,
                    source,
                    coordinate.statement_index as usize,
                    expression,
                )
                .filter(|(statement, subject)| {
                    *statement == coordinate.statement_index as usize
                        && *subject == expression
                        && coordinate.call_ordinal == 0
                })
                .map(|(_, subject)| subject)
            }
            _ => None,
        };
        let place = produced.place;
        let structural_type = produced.structural_type;
        self.evaluation.establish_structural_result(
            self.checked,
            self.state,
            result,
            is_statement_result,
            produced,
            self.structural_types.declarations(),
            self.operations,
        )?;
        if let Some(subject) = saved_subject {
            let root = facts::PlaceRoot::Expression(subject);
            if self
                .evaluation
                .structural_locals
                .iter()
                .any(|binding| binding.root == root)
            {
                return unsupported("computed structural subject was established twice");
            }
            self.evaluation.structural_locals.push(
                crate::expression_preparation::bindings::StructuralLocalBinding {
                    root,
                    structural_type,
                    argument: terminal_psi::StructuralArgument {
                        place,
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::Owned,
                    },
                },
            );
        }
        Ok(())
    }

    /// The source-call row every call records beside its operation.
    pub(super) fn record_source_call(
        &mut self,
        coordinate: checked_trees::CheckedUnitCallCoordinate,
        source_site: Option<checked_trees::NominalMachineUseSite>,
        operation: semantic_vocabulary::OperationId,
        target: symbols::SymbolHandle,
    ) -> Result<(), LoweringError> {
        self.operations.record_source_call(
            SourceCallCoordinate {
                state: self.state,
                statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                    LoweringError::Unsupported("Unit call statement coordinate exceeds usize")
                })?,
                call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                    LoweringError::Unsupported("Unit call ordinal coordinate exceeds usize")
                })?,
            },
            source_site,
            operation,
            target,
        )
    }

    /// A call into a complete Unit body of the closure: operands, claims,
    /// requirement obligations and crash substitution through
    /// `ordinary_calls::prepare`, then either a Unit call or the structural
    /// call that establishes the callee's owned result.
    fn unit_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        inputs: &CallInputs<'_>,
    ) -> Result<(), LoweringError> {
        let checked = self.checked;
        // An ordinary call with a Unit or structural result.
        let Some(call) = operation
            .call()
            .filter(|call| !call.is_boundary() && call.scalar_result().is_none())
        else {
            return unsupported("internal Unit call custody drifted before emission");
        };
        let coordinate = &call.coordinate;
        let target_machine = &call.target.machine;
        let target_state = &call.target.state;
        let structural_arguments = &call.structural_arguments;
        let earlier = self
            .results
            .earlier(structural_arguments, self.operations)?;
        let prepared = ordinary_calls::prepare(
            checked,
            self.callees.plans,
            operation,
            signatures::find(self.callees.signatures, *target_machine)?.call_target(),
            inputs.evaluated_scalar_arguments,
            self.values,
            self.caller.erased_scalar_parameters,
            self.caller.erased_proof_parameters,
            self.parameters,
            self.caller.local_places,
            &earlier,
            self.primitive_locals,
            self.type_ids,
            self.structural_types.declarations(),
            inputs.byte_places,
            self.callees.domain_ids,
            self.caller.claims.bindings(),
            self.calls,
            &self.evaluation.structural_fields,
            self.next_value,
            self.operations,
        )?;
        drop(earlier);
        let callee = lookup_machine_id(self.calls.machine_ids, *target_machine)?;
        if let CheckedUnitCallResult::Structural {
            binding: result,
            discard_result_on_return,
        } = call.result()
        {
            // The dense route registers the value as the call emits it; a
            // composed state registers it, with its local, when publishing.
            let register = matches!(self.results, StructuralResults::Dense(_));
            let declaration = ordinary_calls::emit_structural(
                checked,
                self.state,
                operation,
                prepared,
                callee,
                self.type_ids,
                self.structural_types.declarations(),
                self.callees.domain_ids,
                self.caller.claims.bindings(),
                register,
                self.next_place,
                self.operations,
            )?;
            return self.publish_result(operation, result, declaration, *discard_result_on_return);
        }
        let id = self.operations.allocate();
        self.record_source_call(*coordinate, None, id, *target_state)?;
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: OperationResult::Unit,
            kind: prepared
                .into_direct_call(callee, DirectCallResult::Unit)
                .into_operation_kind(),
        });
        Ok(())
    }

    /// A structural call to a claim-free affine leaf that is not a Unit body
    /// of the closure (`structural_calls::emit`). The leaf's result is not a
    /// completed claim frontier, so the dense route does not register it.
    fn claim_free_structural_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        inputs: &CallInputs<'_>,
    ) -> Result<(), LoweringError> {
        let Some((call, result)) = operation
            .call()
            .filter(|call| !call.is_boundary())
            .and_then(|call| Some((call, call.structural_result()?)))
        else {
            return unsupported("claim-free structural call custody drifted before emission");
        };
        let earlier = self
            .results
            .earlier(&call.structural_arguments, self.operations)?;
        let (declaration, discard_on_return) = structural_calls::emit(
            self.checked,
            self.state,
            operation,
            self.parameters,
            inputs.evaluated_scalar_arguments,
            self.structural_types.declarations(),
            self.type_ids,
            self.calls.machine_ids,
            &earlier,
            self.next_place,
            self.operations,
            Some(self.values),
            &self.evaluation.structural_fields,
            self.next_value,
            Some(&mut *self.calls),
        )?;
        drop(earlier);
        self.publish_result(operation, result, declaration, discard_on_return)
    }

    /// Establish one fresh structural value. Its member operands that are
    /// calls into Unit bodies emit exactly like a statement-level structural
    /// call: `ordinary_calls::prepare` against the callee's signature, then
    /// `emit_structural`, registering the member's own result binding.
    pub(super) fn establish_structural_value(
        self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            discard_result_on_return,
            ..
        } = operation
        else {
            return unsupported("structural value producer missing");
        };
        let OperationFrame {
            checked,
            machine,
            state,
            scalar_result,
            scalar_parameter_count,
            parameters,
            structural_types,
            type_ids,
            service_ids,
            primitive_locals,
            mut results,
            private_places,
            windows,
            evaluation,
            values,
            next_place,
            next_value,
            next_block,
            next_edge,
            calls,
            operations,
            callees,
            mut caller,
            ..
        } = self;
        let structural_types = structural_types.declarations();
        // Fresh linear construction carries the claims the checker
        // established at the binding statement. Resolve them before the
        // operand emitter borrows the caller's claim namespace; the
        // producing operation's result publishes them below.
        let result_claims = if result.multiplicity == Multiplicity::Linear {
            structural_values::establishment_claims(
                checked,
                machine,
                state,
                result,
                &mut caller.claims,
            )?
        } else {
            Vec::new()
        };
        // The evaluator supplies its current namespaces at the exact operand
        // occurrence. Recursive construction and calls share normal frame
        // publication; no stale snapshot or second result home is introduced.
        let mut emit_operand =
            |operand: &CheckedUnitEffectOperationPlan,
             evaluated: Option<&[ValueDeclaration]>,
             source_value_count: usize,
             evaluation: &mut argument_evaluation::Evaluation,
             values: &mut Vec<ValueDeclaration>,
             next_value: &mut u64,
             next_block: &mut u64,
             next_edge: &mut u64,
             calls: &mut CallEmissionContext<'_>,
             operations: &mut OperationBuffer,
             next_place: &mut u64,
             temporary_places: &mut Vec<StructuralPlaceDeclaration>| {
                let result =
                    operand
                        .structural_result_binding()
                        .ok_or(LoweringError::Unsupported(
                            "structural operand preparation has no result",
                        ))?;
                results.require_next(result, "structural operand result binding is not dense")?;
                let frame = OperationFrame {
                    checked,
                    machine,
                    state,
                    scalar_result,
                    scalar_parameter_count,
                    source_value_count,
                    parameters,
                    structural_types: super::StructuralTypeRoster::Published(structural_types),
                    type_ids,
                    service_ids,
                    primitive_locals,
                    results: match &mut results {
                        StructuralResults::Dense(roster) => StructuralResults::Dense(roster),
                        StructuralResults::StateGraph { state, places } => {
                            StructuralResults::StateGraph { state, places }
                        }
                    },
                    private_places: temporary_places,
                    windows: &mut *windows,
                    evaluation,
                    values,
                    next_place,
                    next_value,
                    next_block,
                    next_edge,
                    calls,
                    operations,
                    callees,
                    caller: super::CallerCustody {
                        erased_scalar_parameters: caller.erased_scalar_parameters,
                        erased_proof_parameters: caller.erased_proof_parameters,
                        entry_claims: caller.entry_claims,
                        local_places: caller.local_places,
                        claims: match &mut caller.claims {
                            super::ClaimBindings::Fixed(bindings) => {
                                super::ClaimBindings::Fixed(bindings)
                            }
                            super::ClaimBindings::Growable {
                                bindings,
                                next_claim,
                            } => super::ClaimBindings::Growable {
                                bindings,
                                next_claim,
                            },
                        },
                    },
                };
                if OperationFrame::lowers(operand) {
                    frame.emit(operand)?;
                    return Ok(None);
                }
                frame.emit_call(
                    operand,
                    CallInputs {
                        evaluated_scalar_arguments: evaluated,
                        byte_places: &[],
                        staged: false,
                    },
                )?;
                let place = results.continuation_place(result.binding_ordinal, operations)?;
                let declaration = match &results {
                    StructuralResults::Dense(roster) => {
                        roster.iter().find_map(|(declaration, _)| {
                            (declaration.id == place).then_some(*declaration)
                        })
                    }
                    StructuralResults::StateGraph { places, .. } => places
                        .iter()
                        .find(|declaration| declaration.id == place)
                        .copied(),
                }
                .ok_or(LoweringError::Unsupported(
                    "structural operand result has no declaration",
                ))?;
                Ok(Some(declaration))
            };
        let declaration = structural_values::emit(
            checked,
            machine,
            state,
            operation,
            structural_types,
            type_ids,
            callees.domain_ids,
            next_place,
            private_places,
            &mut emit_operand,
            calls,
            evaluation,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            result_claims,
        )?;
        results.require_next(result, "structural value result binding is not dense")?;
        // A call-rooted value's place was catalogued by the call emitter
        // itself as its own call result; there is no fresh declaration to
        // republish.
        if let Some(declaration) = declaration {
            results.push(declaration, *discard_result_on_return);
        }
        Ok(())
    }

    /// One ordinary scalar call into a prepared scalar callee or a
    /// scalar-completing Unit body. Structural operands or claims select the
    /// structural scalar call shape.
    fn scalar_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        inputs: &CallInputs<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        let checked = self.checked;
        let Some((call, result)) = operation
            .call()
            .filter(|call| !call.is_boundary())
            .and_then(|call| Some((call, call.scalar_result()?)))
        else {
            return unsupported("Unit scalar call custody drifted before emission");
        };
        let coordinate = &call.coordinate;
        let realization_machine = &call.target.machine;
        let realization_state = &call.target.state;
        let source_target = call.target.state;
        if usize::try_from(result.binding_ordinal)
            .ok()
            .and_then(|ordinal| ordinal.checked_add(self.scalar_parameter_count))
            != Some(self.source_value_count)
        {
            return unsupported("Unit scalar result binding ordinal drifted from source order");
        }
        let target = CheckedScalarCallee::find_for_unit_call(checked, *realization_machine)?;
        // Both catalogs have been source-validated before emission.
        // An operation-body callee uses its real ordered emitter,
        // not a fabricated PreparedScalarCallee graph.
        let prepared = self
            .callees
            .prepared_scalar_machines
            .iter()
            .find(|prepared| prepared.source_machine() == *realization_machine);
        // The call result republishes the callee's own declared membership:
        // a prepared graph carries its qualified result; every other callee
        // kind mints an unqualified result, which is exactly what its emitted
        // machine declares. Operands meet the callee's emitted entry
        // parameters the same way — qualified where the callee publishes
        // qualifications, unqualified where it does not.
        let (target_result, result_qualifications) = match prepared {
            Some(PreparedScalarCallee::Graph(graph)) => (
                graph.result_type.scalar_type,
                graph.result_type.qualifications,
            ),
            Some(prepared) => (prepared.result_type(), ScalarQualificationSetId::ZERO),
            None if self.callees.closure.contains(realization_machine)
                && matches!(target, CheckedScalarCallee::Operations(_)) =>
            {
                (target.result_type()?, ScalarQualificationSetId::ZERO)
            }
            None => {
                return unsupported("Unit scalar call target is absent from the prepared closure");
            }
        };
        if target.entry_state()? != *realization_state
            || target_result != terminal_scalar_type(result.primitive_type)?
        {
            return unsupported("Unit scalar call disagrees with its prepared target signature");
        }
        let target_parameter_types = target
            .parameter_types()?
            .iter()
            .map(|primitive| terminal_scalar_type(*primitive))
            .collect::<Result<Vec<_>, _>>()?;
        let expected_parameter_types = match prepared {
            Some(PreparedScalarCallee::Graph(graph)) => graph
                .states
                .first()
                .map(|state| state.parameter_types.clone())
                .ok_or(LoweringError::Unsupported(
                    "scalar call target lost its prepared entry state",
                ))?,
            _ => target_parameter_types
                .iter()
                .copied()
                .map(semantic_vocabulary::QualifiedScalarType::from)
                .collect(),
        };
        let (arguments, erased_arguments, erased_proof_arguments) = match (
            call.erased_scalar_arguments(),
            call.erased_proof_arguments(),
        ) {
            (Some(erased_scalar_arguments), Some(erased_proof_arguments)) => {
                // The checked plan keeps the proof-only lanes apart from the
                // retained operands, in the target's erased-formal order, as
                // for a Unit call: erased actuals are pure terms over the
                // caller's namespace and never runtime operands.
                let arguments = argument_evaluation::validated_values(
                    inputs.evaluated_scalar_arguments,
                    &expected_parameter_types,
                )?;
                if erased_scalar_arguments.len() != target.erased_parameters().len() {
                    return unsupported("scalar call erased lane disagrees with its target roster");
                }
                let erased_arguments = erased_scalar_arguments
                    .iter()
                    .map(|argument| {
                        let checked_trees::CheckedCallScalarArgument::Pure(expression) = argument
                        else {
                            return unsupported(
                                "scalar call erased actual must be a pure checked expression",
                            );
                        };
                        crate::proofs::crash_routes::checked_scalar_term(
                            expression,
                            self.values,
                            self.caller.erased_scalar_parameters,
                        )
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                if erased_proof_arguments.len() != target.erased_proof_parameters().len() {
                    return unsupported("scalar call erased proof roster drifted from its target");
                }
                // Caller `Formal` occurrences resolve against this body's
                // erased-proof roster.
                let erased_proof_arguments = erased_proof_arguments
                    .iter()
                    .map(|term| {
                        crate::scalar_graph::scalar_contracts::checked_proof_term(
                            checked,
                            term,
                            self.caller.erased_proof_parameters,
                        )
                        .and_then(|term| {
                            crate::scalar_graph::scalar_contracts::lowered_proof_term(
                                &term,
                                self.values,
                                self.caller.erased_scalar_parameters,
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                (arguments, erased_arguments, erased_proof_arguments)
            }
            _ => return unsupported("Unit scalar call custody drifted before emission"),
        };
        if checked
            .facts
            .contract_plans
            .for_machine(*realization_machine)
            .is_none()
        {
            return Err(LoweringError::Unsupported(
                "Unit scalar call target has no checked contract",
            ));
        }
        let crash_continuations = lower_checked_crash_route_buckets(
            &crate::unit::effective_crash_routes(checked, *realization_machine)?,
            &arguments,
        )?;
        let requirement_count = self
            .calls
            .requirement_counts
            .iter()
            .find_map(|(source, count)| (*source == *realization_machine).then_some(*count))
            .ok_or(LoweringError::Unsupported(
                "Unit scalar call target has no prepared contract",
            ))?;
        let requirement_obligations = (0..requirement_count)
            .map(|_| self.calls.allocate_requirement())
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let value = ValueDeclaration {
            qualifications: result_qualifications,
            id: value_id(allocate_dense(self.next_value)?),
            scalar_type: target_result,
        };
        let operation_id = self.operations.allocate();
        self.record_source_call(*coordinate, None, operation_id, source_target)?;
        let callee = lookup_machine_id(self.calls.machine_ids, *realization_machine)?;
        let arguments = arguments.iter().map(|argument| argument.id).collect();
        let structural_arguments = &call.structural_arguments;
        let claim_transfers = call.claim_transfers();
        if structural_arguments.len() != target.structural_parameters().len() {
            return unsupported("scalar call arguments disagree with its retained formals");
        }
        // The frame is chosen from the checked call before its operands are
        // lowered: only a structural frame lowers structural arguments.
        let frame = ScalarCallFrame::for_call(
            !structural_arguments.is_empty(),
            !claim_transfers.is_empty(),
            target.requires_structural_frame(),
        );
        let (structural_arguments, claim_transfers) = if frame == ScalarCallFrame::Structural {
            if target.structural_parameters().is_empty() && !structural_arguments.is_empty() {
                return unsupported("structural scalar call has no structural checked body");
            }
            // Arguments and transfer indices already name retained dense slots.
            // Never remove a receiver or rebase claims during emission.
            let mut selector_values = parameters::mint_runtime_index_selectors(
                structural_arguments,
                &self.evaluation.structural_fields,
                self.values,
                self.next_value,
                self.operations,
            )?
            .into_iter();
            let earlier = self
                .results
                .earlier(structural_arguments, self.operations)?;
            parameters::mint_element_view_length_observations(
                structural_arguments,
                self.parameters,
                &earlier,
                self.structural_types.declarations(),
                self.next_value,
                self.operations,
            )?;
            validate_transfer_shape(
                structural_arguments,
                claim_transfers,
                self.parameters,
                self.caller.local_places,
                &earlier,
                target.structural_parameters(),
                self.type_ids,
                self.structural_types.declarations(),
                // A claim-carrying `self` formal fed by a completed result
                // consumes the moved frontier instead of publishing a
                // transfer row; every other entry claim expects exactly one.
                &target
                    .entry_claims()
                    .iter()
                    .filter(|claim| {
                        target
                            .structural_parameters()
                            .get(claim.parameter_index as usize)
                            .zip(structural_arguments.get(claim.parameter_index as usize))
                            .is_none_or(|(parameter, argument)| {
                                !parameter.is_self
                                    || argument
                                        .source_structural_result_binding_ordinal()
                                        .is_none()
                            })
                    })
                    .map(|claim| claim.parameter_index)
                    .collect::<Vec<_>>(),
                self.primitive_locals,
                Some(StructuralResultCustody {
                    results: &self.operations.structural_values,
                    domains: self.callees.domain_ids,
                    claims: self.caller.claims.bindings(),
                    target_entry_claims: target.entry_claims(),
                }),
            )?;
            (
                lower_structural_arguments(
                    structural_arguments,
                    self.structural_types.declarations(),
                    self.parameters,
                    self.caller.local_places,
                    &earlier,
                    inputs.byte_places,
                    self.primitive_locals,
                    Some(self.values),
                    &mut selector_values,
                    Some(&mut *self.calls),
                )?,
                emitted_claim_transfers(
                    structural_arguments,
                    claim_transfers,
                    target.structural_parameters(),
                    &self.operations.structural_values,
                    self.caller.claims.bindings(),
                )?,
            )
        } else {
            (Vec::new(), Vec::new())
        };
        let kind = DirectCall {
            callee,
            arguments,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            result: DirectCallResult::Scalar(frame),
        }
        .into_operation_kind();
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id,
            result: OperationResult::Scalar(value),
            kind,
        });
        Ok(value)
    }
}
