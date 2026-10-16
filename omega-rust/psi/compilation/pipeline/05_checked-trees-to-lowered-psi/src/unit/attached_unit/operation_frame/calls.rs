//! Calls into the closure's own bodies: internal Unit and structural calls,
//! the member calls a structural-value construction makes, and scalar calls.
//! Every module call with a Unit or structural result targets a Unit body of
//! the closure. `emit_call` is the one entry both routes use;
//! boundary calls continue in `boundary_calls`.
//!
//! Every call publishes its result through the frame's representations:
//! a scalar result through `bind_scalar_result` (the dense value namespace
//! plus a composed state's binding namespace), and a structural result
//! through `publish_result` (the dense roster with its local binding, or the
//! composed catalog and operation-buffer registry).
//!
//! Suspension retention joins checked source positions to those same emitted
//! bindings. A scalar whose address was taken, or which permits replacement,
//! lives in its established home rather than its initializer SSA value. Retain
//! that current home before considering unaddressed scalar bindings; creating
//! another home here would split storage identity from later reads and writes.

use super::super::{argument_evaluation, ordinary_calls, signatures, structural_values};
use super::{OperationFrame, StructuralResults};
use crate::direct_calls::DirectCallResult;
use crate::emission::operation_emission::buffer::{OperationBuffer, SourceCallCoordinate};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::scalar_lowering::call_closure::callee::CheckedScalarCallee;
use crate::terminal_identities::lookup_type_id;
use crate::unit::{
    CheckedUnitEffectOperationPlan, LoweringError, OperationResult, PlaceId,
    StructuralPlaceDeclaration, StructuralPlaceKind, ValueDeclaration, allocate_dense,
    lookup_machine_id, terminal_scalar_type, unsupported, value_id,
};
use checked_trees::{CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute};

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
    /// Address operands captured by the same authored schedule as scalar arguments.
    pub(in crate::unit::attached_unit) evaluated_selectors: Option<&'i [ValueDeclaration]>,
}

impl OperationFrame<'_, '_> {
    /// Emit one call and publish its result.
    pub(in crate::unit::attached_unit) fn emit_call(
        mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        operation: &CheckedUnitEffectOperationPlan,
        inputs: CallInputs<'_>,
    ) -> Result<(), LoweringError> {
        let Some(call) = operation.call() else {
            return unsupported("operation frame call entry received a non-call operation");
        };
        // Address demand is independent of the call's result or route. Publish
        // immutable scalar backing before lowering any structural operand, just
        // as explicit reference construction and storage observations do.
        for argument in &call.structural_arguments {
            self.evaluation.prepare_borrowed_argument(
                identities,
                self.checked,
                self.state,
                argument,
                self.values,
                self.structural_types.declarations(),
                self.next_place,
                self.operations,
                self.private_places,
            )?;
        }
        // The route and result select the emitter; each receives the call
        // plan and the result binding this match already destructured.
        match (call.route(), call.result()) {
            (
                CheckedUnitCallRoute::Module { .. },
                CheckedUnitCallResult::Unit | CheckedUnitCallResult::Structural { .. },
            ) => {
                self.unit_call(identities, operation, call, &inputs)?;
                Ok(())
            }
            (CheckedUnitCallRoute::Module { .. }, CheckedUnitCallResult::Scalar(result)) => {
                let value = self.scalar_call(identities, operation, call, result, &inputs)?;
                self.bind_scalar_result(identities, call.coordinate, value)
            }
            (CheckedUnitCallRoute::Boundary, CheckedUnitCallResult::Unit) => {
                self.boundary_call(identities, call, &inputs)?;
                Ok(())
            }
            (CheckedUnitCallRoute::Boundary, CheckedUnitCallResult::Scalar(result)) => {
                let value = self.boundary_scalar_call(identities, call, result, &inputs)?;
                self.bind_scalar_result(identities, call.coordinate, value)
            }
            (
                CheckedUnitCallRoute::Boundary,
                CheckedUnitCallResult::Structural {
                    binding,
                    discard_result_on_return,
                },
            ) => {
                self.boundary_structural_call(
                    identities,
                    operation,
                    call,
                    binding,
                    *discard_result_on_return,
                    &inputs,
                )?;
                Ok(())
            }
            (
                CheckedUnitCallRoute::Dynamic {
                    interface_identity, ..
                },
                _,
            ) => self.dynamic_call(identities, operation, call, interface_identity, &inputs),
        }?;
        self.publish_loan_completions(identities, call)
    }

    /// A retained scalar call result is the next dense value, so it also
    /// enters a composed state's binding namespace at its checked ordinal,
    /// exactly like an established scalar local. Only a result that
    /// initializes a declared immutable local also takes an authored slot;
    /// one an assignment stores stays unnamed, or every later local's
    /// authored read would resolve to it. A discarded result keeps its typed
    /// operation result without occupying a source position.
    pub(super) fn bind_scalar_result(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        coordinate: checked_trees::CheckedUnitCallCoordinate,
        value: ValueDeclaration,
    ) -> Result<(), LoweringError> {
        let names_local =
            crate::call_sites::initializes_immutable_local(self.checked, self.state, coordinate);
        let value = if names_local {
            self.refine_scalar_local_value(identities, coordinate.statement_index, value)?
        } else {
            value
        };
        if !crate::call_sites::discards_result(self.checked, self.state, coordinate) {
            let position = self.values.len();
            self.values.push(value);
            if names_local {
                self.evaluation
                    .scalar_bindings
                    .append(value.scalar_type, position);
            } else {
                self.evaluation
                    .scalar_bindings
                    .append_unnamed_result(position);
            }
        }
        Ok(())
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
        self.publish_completed_result(operation, result, declaration, produced, discard_on_return)
    }

    /// Publish the completed value, whether an operation produced it directly
    /// or a selected call arrived through an ordinary structural block argument.
    /// The join transports the selected leaf's custody; it does not execute a
    /// second call or establish the result's qualifications again.
    pub(super) fn publish_completed_result(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        declaration: StructuralPlaceDeclaration,
        produced: terminal_psi::StructuralOperationResult,
        discard_on_return: bool,
    ) -> Result<(), LoweringError> {
        self.results.push(declaration, discard_on_return);
        if matches!(self.results, StructuralResults::Dense(_)) {
            structural_values::bind_local(
                operation,
                declaration,
                produced.structural_type,
                self.structural_types.declarations(),
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
                let local = result.local;
                if local.is_named()
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
                    self.evaluation.view_locals.push(
                        crate::expression_preparation::bindings::view_locals::ViewLocalBinding {
                            symbol: local.symbol,
                            place: declaration.id,
                            structural_type: produced.structural_type,
                            carrier,
                            access: crate::expression_preparation::bindings::structural_paths::terminal_access(
                                local.access,
                            ),
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
        let authored = crate::call_sites::locate(self.checked, self.state, coordinate)?;
        let (_, source) = crate::declarations::state_declaration(self.checked, self.state)?;
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
        if matches!(declaration.kind, StructuralPlaceKind::BlockParameter { .. }) {
            // The call emitter has prepared the public result contract and
            // recorded the selected leaves' introductions. The completed
            // value now belongs to their ordinary join, not to any one leaf
            // operation. Register that arrival as value selections do; no
            // new claims or memberships are introduced by the local binding.
            super::structural_results::validate_completed_arrival(&declaration, self.evaluation)?;
            self.evaluation.establish_structural_binding(
                result,
                produced.into(),
                self.structural_types.declarations(),
                self.operations,
            )?;
        } else {
            self.evaluation.establish_structural_result(
                self.state,
                result,
                produced,
                self.structural_types.declarations(),
                self.operations,
            )?;
        }
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
                    result_binding_ordinal: Some(result.binding_ordinal),
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

    /// The source-call coordinate and suspension frontier every call records
    /// beside its operation.
    pub(super) fn source_call_record(
        &self,
        coordinate: checked_trees::CheckedUnitCallCoordinate,
    ) -> Result<
        (
            SourceCallCoordinate,
            lowered_psi::LoweredSourceFrontierBindings,
        ),
        LoweringError,
    > {
        Ok((
            SourceCallCoordinate {
                state: self.state,
                statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                    LoweringError::Unsupported("Unit call statement coordinate exceeds usize")
                })?,
                call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                    LoweringError::Unsupported("Unit call ordinal coordinate exceeds usize")
                })?,
            },
            self.suspension_source_frontier(coordinate)?,
        ))
    }

    /// The source-call row every call records beside its operation.
    pub(super) fn record_source_call(
        &mut self,
        coordinate: checked_trees::CheckedUnitCallCoordinate,
        source_site: Option<checked_trees::NominalMachineUseSite>,
        operation: semantic_vocabulary::OperationId,
        target: symbols::SymbolHandle,
    ) -> Result<(), LoweringError> {
        let (coordinate, frontier) = self.source_call_record(coordinate)?;
        self.operations
            .record_source_call(coordinate, source_site, operation, target, &frontier)
    }

    /// Join demanded source bindings through this state's authored namespace.
    /// Checked coordinates count every parameter and local; scalar values and
    /// structural storage have independent emitted namespaces. Preserve each
    /// existing storage home, projected alias and claim identity rather than
    /// renumbering source positions or recreating a value at the crossing.
    /// Non-suspending calls need no frontier companion.
    fn suspension_source_frontier(
        &self,
        coordinate: checked_trees::CheckedUnitCallCoordinate,
    ) -> Result<lowered_psi::LoweredSourceFrontierBindings, LoweringError> {
        let Some(crossing) =
            self.checked
                .facts
                .carry
                .suspension_crossings
                .iter()
                .find(|crossing| {
                    crossing.machine == self.machine
                        && crossing.state == self.state
                        && crossing.statement_index == coordinate.statement_index as usize
                        && crossing.call_ordinal == coordinate.call_ordinal as usize
                })
        else {
            return Ok(Default::default());
        };
        let (_, state) = self
            .checked
            .state_flow_site(self.machine, self.state)
            .ok_or(LoweringError::Unsupported(
                "suspension call lost its checked source state",
            ))?;
        let parameters = self.checked.state_parameters(state);
        let locals = self.checked.facts.state_locals.locals(self.state);
        let mut bindings = lowered_psi::LoweredSourceFrontierBindings {
            claims: self.calls.claims.bindings().to_vec(),
            ..Default::default()
        };
        for capture in &self.operations.loan_captures {
            if self
                .checked
                .facts
                .carry
                .suspension_loans
                .span_or_empty(crossing.borrows.loans)
                .iter()
                .any(|entry| entry.retained_for_crossing && entry.stored_loan() == capture.loan)
            {
                bindings.loan_captures.push(capture.clone());
            }
        }
        for live in &crossing.live_values {
            let (symbol, source_position) = match live.origin {
                checked_trees::SuspensionCrossingValueOrigin::Parameter { symbol, position } => {
                    if !parameters
                        .get(position)
                        .is_some_and(|parameter| parameter.symbol == symbol)
                    {
                        return unsupported(
                            "suspension parameter changes its checked source position",
                        );
                    }
                    (symbol, position)
                }
                checked_trees::SuspensionCrossingValueOrigin::Local {
                    symbol,
                    statement_index,
                    environment_position,
                } => {
                    if !locals.iter().enumerate().any(|(position, local)| {
                        local.symbol == symbol
                            && local.statement_index as usize == statement_index
                            && local.statement_index < coordinate.statement_index
                            && parameters.len() + position == environment_position
                    }) {
                        return unsupported("suspension local changes its checked source position");
                    }
                    (symbol, environment_position)
                }
                checked_trees::SuspensionCrossingValueOrigin::CallArgument { .. } => continue,
            };
            // Storage identity comes before carrier classification. An ordinary
            // scalar binding may now occupy an addressable home; retaining its
            // initializer SSA value would lose intervening writes and its address.
            let local = crate::expression_preparation::bindings::structural_local_by_root(
                &self.evaluation.structural_locals,
                facts::PlaceRoot::Symbol(symbol),
            );
            let home = if let Some(local) = local {
                Some((local.argument.clone(), local.structural_type))
            } else if let Some((_, place, scalar_type)) = self
                .evaluation
                .primitive_storage
                .iter()
                .find(|(candidate, _, _)| *candidate == symbol)
            {
                let declaration = self
                    .primitive_locals
                    .iter()
                    .chain(&self.evaluation.parameter_storage)
                    .find(|local| local.declaration.id == *place)
                    .ok_or(LoweringError::Unsupported(
                        "suspension scalar home has no established declaration",
                    ))?;
                let (_, structural_type) =
                    declaration.declaration.kind.as_operation_result().ok_or(
                        LoweringError::Unsupported(
                            "suspension scalar home has no establishing operation",
                        ),
                    )?;
                let primitive = self
                    .checked
                    .primitive_type_reference(live.type_reference)
                    .ok_or(LoweringError::Unsupported(
                        "suspension scalar home has no checked carrier",
                    ))?;
                if terminal_scalar_type(primitive)? != *scalar_type {
                    return unsupported("suspension scalar home changes its checked carrier");
                }
                Some((
                    terminal_psi::StructuralArgument {
                        place: *place,
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::Owned,
                    },
                    structural_type,
                ))
            } else if self
                .checked
                .primitive_type_reference(live.type_reference)
                .is_none()
            {
                self.evaluation
                    .structural_parameters
                    .iter()
                    .find(|(position, _)| *position as usize == source_position)
                    .map(|(_, parameter)| {
                        (
                            terminal_psi::StructuralArgument {
                                place: parameter.place,
                                path: Vec::new(),
                                access: parameter.access,
                            },
                            parameter.structural_type,
                        )
                    })
            } else {
                None
            };
            if let Some((mut argument, structural_type)) = home {
                argument.place = self.evaluation.current_structural_place(argument.place);
                bindings
                    .structural
                    .push(lowered_psi::LoweredSourceStructuralBinding {
                        source_position,
                        argument,
                        structural_type,
                    });
                continue;
            }
            let authored_position = match live.origin {
                checked_trees::SuspensionCrossingValueOrigin::Parameter { position, .. } => {
                    let parameter = &parameters[position];
                    if parameter.is_mutable
                        || parameter.relevance.is_erased()
                        || self
                            .checked
                            .primitive_type_reference(parameter.type_reference)
                            .is_none()
                    {
                        continue;
                    }
                    parameters[..position]
                        .iter()
                        .filter(|parameter| {
                            !parameter.relevance.is_erased()
                                && self
                                    .checked
                                    .primitive_type_reference(parameter.type_reference)
                                    .is_some()
                        })
                        .count()
                }
                checked_trees::SuspensionCrossingValueOrigin::Local {
                    environment_position,
                    ..
                } => {
                    let local = &locals[environment_position - parameters.len()];
                    if !local.is_scalar_value {
                        continue;
                    }
                    self.scalar_parameter_count
                        + self
                            .checked
                            .facts
                            .state_locals
                            .scalar_locals_before(self.state, local.statement_index)
                            as usize
                }
                checked_trees::SuspensionCrossingValueOrigin::CallArgument { .. } => continue,
            };
            let Some((value_position, scalar_type)) = self
                .evaluation
                .scalar_bindings
                .unaddressed_value(authored_position)
            else {
                continue;
            };
            let value =
                self.values
                    .get(value_position)
                    .copied()
                    .ok_or(LoweringError::Unsupported(
                        "suspension scalar binding lost its emitted value",
                    ))?;
            if value.scalar_type != scalar_type {
                return unsupported("suspension scalar binding changes its carrier");
            }
            bindings
                .scalars
                .push(lowered_psi::LoweredSourceScalarBinding {
                    source_position,
                    value,
                });
        }
        Ok(bindings)
    }

    /// A call into a complete Unit body of the closure: operands, claims,
    /// requirement obligations and crash substitution through
    /// `ordinary_calls::prepare`, then either a Unit call or the structural
    /// call that establishes the callee's owned result.
    fn unit_call(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        operation: &CheckedUnitEffectOperationPlan,
        call: &CheckedUnitCallPlan,
        inputs: &CallInputs<'_>,
    ) -> Result<(), LoweringError> {
        let checked = self.checked;
        let coordinate = &call.coordinate;
        let target_machine = &call.target.machine;
        let target_state = &call.target.state;
        let structural_arguments = &call.structural_arguments;
        let earlier =
            self.results
                .earlier(structural_arguments, self.operations, self.evaluation)?;
        let prepared = ordinary_calls::prepare(
            identities,
            checked,
            self.state,
            operation,
            signatures::find(self.callees.signatures, *target_machine)?.call_target(),
            inputs.evaluated_scalar_arguments,
            inputs.evaluated_selectors,
            self.values,
            self.caller.erased_scalar_parameters,
            self.caller.erased_proof_parameters,
            &self.parameters,
            self.caller.local_places,
            &earlier,
            self.primitive_locals,
            &self.evaluation.structural_locals,
            &self.evaluation.scalar_bindings,
            self.structural_types.declarations(),
            inputs.byte_places,
            self.calls,
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
            let frontier = self.suspension_source_frontier(*coordinate)?;
            let declaration = ordinary_calls::emit_structural(
                identities,
                checked,
                self.state,
                operation,
                prepared,
                callee,
                self.type_ids,
                self.structural_types.declarations(),
                self.callees.domain_ids,
                &mut self.calls.claims,
                register,
                &frontier,
                self.next_place,
                self.operations,
            )?;
            return self.publish_result(operation, result, declaration, *discard_result_on_return);
        }
        let (coordinate, frontier) = self.source_call_record(*coordinate)?;
        prepared.emit(
            self.operations,
            coordinate,
            None,
            *target_state,
            crate::direct_calls::CallRecord::Frontier(&frontier),
            OperationResult::Unit,
            callee,
            DirectCallResult::Unit,
        )?;
        Ok(())
    }

    /// Establish one fresh structural value. Its member operands that are
    /// calls into Unit bodies emit exactly like a statement-level structural
    /// call: `ordinary_calls::prepare` against the callee's signature, then
    /// `emit_structural`, registering the member's own result binding.
    pub(super) fn establish_structural_value(
        self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                result,
                value,
                discard_result_on_return,
                ..
            },
        ) = operation
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
            caller,
            ..
        } = self;
        let structural_types = structural_types.declarations();
        // Fresh construction carries the claims the checker established at
        // the binding statement. Resolve them before the operand emitter
        // borrows the caller's claim namespace; the producing operation's
        // result publishes them below.
        //
        // The binding's own multiplicity does not decide whether there are
        // claims to carry. A transparent affine aggregate contributes no root
        // claim and holds each contained linear member as an independent
        // path-indexed claim, so an affine result can own several linear
        // claims and a linear result owns exactly its whole-root one.
        // A completed value has its own expression root unless the exact
        // local initializer receives it. Use the retained value occurrence,
        // not an assumption that every structural producer is a declaration.
        let occurrence = checked.facts.values.structural_values.nodes.get(*value);
        let owner = if result.local.is_named() {
            facts::PlaceRoot::Symbol(result.local.symbol)
        } else {
            facts::PlaceRoot::Expression(occurrence.expression)
        };
        let prepared_claims = crate::emission::claim_bindings::prepare_establishment_claims(
            identities,
            checked,
            machine,
            state,
            result.statement_index as usize,
            owner,
            occurrence.type_reference,
            lookup_type_id(type_ids, &result.type_identity)?,
            structural_types,
        )?;
        let result_claims = crate::emission::claim_bindings::emit_establishment_claims(
            &prepared_claims,
            &mut calls.claims,
        )?;
        // The evaluator supplies its current namespaces at the exact operand
        // occurrence. Recursive construction and calls share normal frame
        // publication; no stale snapshot or second result home is introduced.
        let mut emit_operand =
            |identities: &mut terminal_psi::TerminalIdentityTableBuilder,
             operand: &CheckedUnitEffectOperationPlan,
             evaluated: Option<&[ValueDeclaration]>,
             evaluated_selectors: Option<&[ValueDeclaration]>,
             byte_places: &[PlaceId],
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
                    parameters: evaluation.current_parameters(&parameters),
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
                        local_places: caller.local_places,
                    },
                };
                if OperationFrame::lowers(operand) {
                    frame.emit(identities, operand)?;
                    return Ok(None);
                }
                frame.emit_call(
                    identities,
                    operand,
                    CallInputs {
                        evaluated_scalar_arguments: evaluated,
                        byte_places,
                        evaluated_selectors,
                    },
                )?;
                let place = results.continuation_place(result.binding_ordinal, operations)?;
                let declaration = match &results {
                    StructuralResults::Dense(roster) => {
                        roster.iter().find_map(|(declaration, _)| {
                            declaration.filter(|declaration| declaration.id == place)
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
        let direct_establishments = crate::emission::claim_bindings::direct_establishment_claims(
            &prepared_claims,
            &calls.claims,
        )?;
        let declaration = structural_values::emit(
            identities,
            checked,
            machine,
            state,
            operation,
            structural_types,
            type_ids,
            callees.domain_ids,
            callees.domain_declarations,
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
            direct_establishments,
        )?;
        results.publish_value_binding(
            result,
            declaration,
            *discard_result_on_return,
            operations,
        )?;
        Ok(())
    }

    /// One ordinary scalar call into a prepared scalar callee or a
    /// scalar-completing Unit body. Structural operands or claims select the
    /// structural scalar call shape.
    fn scalar_call(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        operation: &CheckedUnitEffectOperationPlan,
        call: &CheckedUnitCallPlan,
        result: &checked_trees::CheckedUnitScalarResultBindingPlan,
        inputs: &CallInputs<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        let checked = self.checked;
        let coordinate = &call.coordinate;
        let realization_machine = &call.target.machine;
        let realization_state = &call.target.state;
        let source_target = call.target.state;
        let target = CheckedScalarCallee::find_for_unit_call(checked, *realization_machine)?;
        if !self.callees.closure.contains(realization_machine) {
            return unsupported("Unit scalar call target is absent from the prepared closure");
        }
        // The call result republishes the callee's own declared membership,
        // and operands meet the callee's declared entry formals: its
        // allocated `MachineSignature` carries both, tags included.
        let signature =
            super::super::signatures::find(self.callees.signatures, *realization_machine)?;
        let declared_result = signature.scalar_result.ok_or(LoweringError::Unsupported(
            "Unit scalar call target declares no scalar result",
        ))?;
        let (target_result, result_qualifications) =
            (declared_result.scalar_type, declared_result.qualifications);
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
        let expected_parameter_types = signature
            .scalar_parameters
            .iter()
            .map(|parameter| parameter.value_type())
            .collect::<Vec<_>>();
        if expected_parameter_types.len() != target_parameter_types.len()
            || expected_parameter_types
                .iter()
                .zip(&target_parameter_types)
                .any(|(declared, carrier)| declared.scalar_type != *carrier)
        {
            return unsupported("Unit scalar call target formals disagree with its signature");
        }
        if call
            .erased_scalar_arguments()
            .ok_or(LoweringError::Unsupported(
                "scalar module call has no erased lane",
            ))?
            .len()
            != target.erased_parameters().len()
        {
            return unsupported("scalar call erased lane disagrees with its target roster");
        }
        // Structural guards resolve through the same allocated signature and
        // completed operands as Unit calls. A scalar result changes publication,
        // never the namespace or root substitution of the callee's contract.
        let earlier =
            self.results
                .earlier(&call.structural_arguments, self.operations, self.evaluation)?;
        let prepared = ordinary_calls::prepare(
            identities,
            checked,
            self.state,
            operation,
            signature.call_target(),
            inputs.evaluated_scalar_arguments,
            inputs.evaluated_selectors,
            self.values,
            self.caller.erased_scalar_parameters,
            self.caller.erased_proof_parameters,
            &self.parameters,
            self.caller.local_places,
            &earlier,
            self.primitive_locals,
            &self.evaluation.structural_locals,
            &self.evaluation.scalar_bindings,
            self.structural_types.declarations(),
            inputs.byte_places,
            self.calls,
            self.next_value,
            self.operations,
        )?;
        drop(earlier);
        let value = ValueDeclaration {
            qualifications: result_qualifications,
            id: value_id(allocate_dense(self.next_value)?),
            scalar_type: target_result,
        };
        let callee = lookup_machine_id(self.calls.machine_ids, *realization_machine)?;
        let (coordinate, frontier) = self.source_call_record(*coordinate)?;
        prepared.emit(
            self.operations,
            coordinate,
            None,
            source_target,
            crate::direct_calls::CallRecord::Frontier(&frontier),
            OperationResult::Scalar(value),
            callee,
            DirectCallResult::Scalar,
        )?;
        Ok(value)
    }
}
