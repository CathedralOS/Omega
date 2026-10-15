use super::{Builder, CheckedScalarExpressionRole, ExpressionHandle, SymbolHandle};
use crate::values::scalar::call_lowering::call_is_boundary;

impl Builder<'_, '_> {
    /// Index operands have mathematical carriers, not necessarily stored type
    /// references: a binary result owns no declaration identity. Reuse the
    /// integer evaluator so computed selectors preserve signedness and policy;
    /// only an anonymous integer needs the ordinary u64 index context.
    pub(super) fn index_selector(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<checked_trees::CheckedScalarComputationHandle> {
        let operand = self.integer_operand(expression, None)?;
        let value = self.materialize_integer(operand)?;
        let value = if self
            .plans
            .nodes
            .get(value)
            .primitive_type
            .is_fixed_integer()
        {
            value
        } else {
            self.expression(expression, typed_trees::types::PrimitiveType::U64)?
        };
        self.plans.nodes.get_mut(value).authored_root = expression;
        Some(value)
    }

    pub(super) fn record_transition_selectors(
        &mut self,
        statement: u32,
        argument_ordinal: u32,
        argument: ExpressionHandle,
    ) {
        let Some(place) = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            argument,
        ) else {
            return;
        };
        for segment in place.segments {
            if let facts::PlaceSegment::Index { expression } = segment {
                let role = CheckedScalarExpressionRole::TransitionSelector {
                    argument_ordinal,
                    expression,
                };
                if self.plans.root_at(self.state, statement, role).is_none()
                    && let Some(root) = self.index_selector(expression)
                {
                    self.push_scalar_root(role, root);
                }
            }
        }
    }

    pub(super) fn record_call_arguments(
        &mut self,
        statement: u32,
        call_ordinal: u32,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
    ) {
        if let Some(operation) = self
            .program
            .symbols
            .builtin_function_for_symbol(target)
            .and_then(|builtin| {
                language_core::inline_assembly::AsmRegisterOperation::from_intrinsic_name(
                    builtin.name(),
                )
            })
        {
            if arguments.len() != operation.input_carriers().len() {
                return;
            }
            for (ordinal, (argument, carrier)) in
                arguments.iter().zip(operation.input_carriers()).enumerate()
            {
                let (Ok(argument_ordinal), Some(primitive_type)) = (
                    u32::try_from(ordinal),
                    typed_trees::types::PrimitiveType::from_name(carrier.name()),
                ) else {
                    return;
                };
                self.record_root(
                    self.pure,
                    statement,
                    CheckedScalarExpressionRole::UnitCallArgument {
                        call_ordinal,
                        argument_ordinal,
                    },
                    *argument,
                    primitive_type,
                );
            }
            return;
        }
        let Some(parameters) = validation::call_target_parameters(self.program, target) else {
            return;
        };
        let explicit_parameters = parameters.iter();
        if parameters.len() != arguments.len() {
            return;
        }
        let boundary = call_is_boundary(self.program, target);
        let mut scalar_ordinal = 0u32;
        let mut structural_ordinal = 0u32;
        for (argument, parameter) in arguments.iter().zip(explicit_parameters) {
            // An erased parameter position has no scalar ordinal: the callee
            // plan (`execution::unit::calls`) skips it the same way.
            match crate::execution::terminal_unit::strips_erased_parameter(parameter) {
                Some(true) => continue,
                Some(false) => {}
                None => return,
            }
            let Some(primitive_type) = self
                .program
                .primitive_type_reference(parameter.type_reference)
            else {
                // The shared value owner retains construction and copying at
                // their exact argument occurrence. Lending admission remains
                // separate; no source-shape roster decides what a value is.
                if !parameter.is_const {
                    self.retain_structural_argument(*argument, parameter.type_reference);
                    if let Some(place) = validation::canonical_place_from_expression_in_state(
                        self.program,
                        self.state,
                        self.statement_index,
                        *argument,
                    ) {
                        for segment in place.segments {
                            if let facts::PlaceSegment::Index { expression } = segment {
                                let role = CheckedScalarExpressionRole::CallSelector {
                                    call_ordinal,
                                    argument_ordinal: structural_ordinal,
                                    expression,
                                };
                                if self.plans.root_at(self.state, statement, role).is_none()
                                    && let Some(root) = self.index_selector(expression)
                                {
                                    self.push_scalar_root(role, root);
                                }
                            }
                        }
                    }
                    let Some(next) = structural_ordinal.checked_add(1) else {
                        return;
                    };
                    structural_ordinal = next;
                }
                continue;
            };
            if parameter.is_const
                || (parameter.is_mutable
                    && crate::values::mutable_scalar_parameter_type(self.program, parameter)
                        .is_none())
            {
                return;
            }
            let role = if boundary {
                CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal,
                    argument_ordinal: scalar_ordinal,
                }
            } else {
                CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal,
                    argument_ordinal: scalar_ordinal,
                }
            };
            self.record_root(self.pure, statement, role, *argument, primitive_type);
            let Some(next) = scalar_ordinal.checked_add(1) else {
                return;
            };
            scalar_ordinal = next;
        }
    }
}

// Call operands have one evaluator regardless of whether the eventual invocation
// enters a checked body or a boundary. The exact formal roster determines the
// scalar/structural lanes; the producer retains authored order and loan custody.
impl Builder<'_, '_> {
    pub(super) fn call_operands(
        &mut self,
        source_call: arena::Handle<checked_trees::FlowCallFact>,
        arguments: &[ExpressionHandle],
        parameters: &[typed_trees::signature::StateParameter],
    ) -> Option<ComputedCallOperands> {
        use typed_trees::types::TypeReferenceNode;
        if arguments.len() != parameters.len()
            || parameters.iter().any(|parameter| {
                parameter.is_const
                    || (parameter.is_mutable
                        && self
                            .program
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                        && crate::values::mutable_scalar_parameter_type(self.program, parameter)
                            .is_none())
            })
        {
            return None;
        }
        let mut computed_arguments = Vec::with_capacity(arguments.len());
        let mut structural_arguments = Vec::new();
        let mut structural_actuals = Vec::new();
        let mut temporary_cleanups = Vec::new();
        for (argument, parameter) in arguments.iter().zip(parameters) {
            // An erased parameter position consumes its authored
            // argument as proof material only: no scalar operand and
            // no structural argument, matching the callee's stripped
            // signature plan (`execution::unit::calls::signatures`).
            if crate::execution::terminal_unit::strips_erased_parameter(parameter)? {
                continue;
            }
            if let Some(primitive_type) = self
                .program
                .primitive_type_reference(parameter.type_reference)
            {
                let root = self.expression(*argument, primitive_type)?;
                self.plans.nodes.get_mut(root).authored_root = *argument;
                computed_arguments.push(root);
            } else {
                let state =
                    validation::find_state_in_machine(self.program, self.machine, self.state)?;
                // Owned operands consume the established value, not a live
                // view of storage which a later argument can replace. Borrowed
                // operands keep their loan route; only computed temporaries
                // need an established owner on that route.
                let reference_formal = matches!(
                    self.program
                        .type_reference_table
                        .type_reference(parameter.type_reference),
                    TypeReferenceNode::Reference { .. }
                );
                // Binding mutability controls writes inside the callee;
                // every owned actual still establishes an independent value.
                let retained = if !reference_formal {
                    self.retain_structural_argument(*argument, parameter.type_reference)
                } else {
                    None
                };
                let argument_ordinal = u32::try_from(structural_arguments.len()).ok()?;
                let argument_plan = if let Some(value) = retained {
                    self.retained_call_argument(
                        source_call,
                        *argument,
                        parameter,
                        value,
                        argument_ordinal,
                        &mut temporary_cleanups,
                    )?
                } else {
                    (|| {
                        let mut selectors = Vec::new();
                        if let Some(place) = validation::canonical_place_from_expression_in_state(
                            self.program,
                            self.state,
                            self.statement_index,
                            *argument,
                        ) {
                            for segment in place.segments {
                                if let facts::PlaceSegment::Index { expression } = segment {
                                    let value = self.index_selector(expression)?;
                                    selectors.push((
                                        expression,
                                        checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(value),
                                    ));
                                }
                            }
                        }
                        crate::execution::terminal_unit::structural_computation_argument(
                            self.program,
                            self.borrow,
                            self.machine,
                            state,
                            self.flow.control.calls.get(source_call),
                            *argument,
                            parameter,
                            Some(&|expression| {
                                selectors.iter().find_map(|(source, value)| {
                                    (*source == expression).then(|| value.clone())
                                })
                            }),
                        )
                    })()
                    .or_else(|| {
                        let argument_ordinal = structural_arguments.len();
                        crate::execution::terminal_unit::calls::subslice_computation_argument(
                            self.program,
                            self.operators,
                            self.pure,
                            self.plans,
                            self.borrow,
                            self.machine,
                            state,
                            self.flow.control.calls.get(source_call),
                            *argument,
                            parameter,
                            u32::try_from(argument_ordinal).ok()?,
                        )
                    })
                    .or_else(|| {
                        // A `"literal"` actual loans a byte sequence the
                        // callee reads as a borrowed view; places alone
                        // cannot name it, so the call argument reuses the
                        // statement route's literal carrier.
                        crate::execution::terminal_unit::calls::byte_sequence_literal_argument(
                            self.program,
                            parameter.type_reference,
                            *argument,
                        )
                    })
                    .or_else(|| {
                        if !reference_formal || parameter.is_mutable {
                            return None;
                        }
                        let value =
                            self.retain_structural_argument(*argument, parameter.type_reference)?;
                        self.retained_call_argument(
                            source_call,
                            *argument,
                            parameter,
                            value,
                            argument_ordinal,
                            &mut temporary_cleanups,
                        )
                    })?
                };
                structural_actuals.push(*argument);
                structural_arguments.push(
                    checked_trees::CheckedScalarComputationStructuralArgument::Place(argument_plan),
                );
            }
        }
        // The claims crossing at this call join the same owned operands an
        // ordinary call operation's arguments join.
        let claim_transfers =
            crate::execution::terminal_unit::calls::computed_call_claim_transfers(
                self.program,
                self.flow,
                self.machine,
                self.state,
                self.flow.control.calls.get(source_call),
                &structural_actuals
                    .iter()
                    .zip(&structural_arguments)
                    .map(|(actual, argument)| Some((*actual, argument.as_place()?)))
                    .collect::<Option<Vec<_>>>()?,
            )?;
        let arguments = self.plans.operands.insert_many(computed_arguments);
        let structural_arguments = self
            .plans
            .structural_arguments
            .insert_many(structural_arguments);
        temporary_cleanups.reverse();
        let temporary_cleanups = self
            .plans
            .temporary_cleanups
            .insert_many(temporary_cleanups);
        let claim_transfers = self.plans.claim_transfers.insert_many(claim_transfers);
        Some(ComputedCallOperands {
            arguments,
            structural_arguments,
            temporary_cleanups,
            claim_transfers,
        })
    }

    /// Consume an established producer. A failed owner or custody join cannot
    /// turn this operand back into a live source-place argument.
    fn retained_call_argument(
        &mut self,
        source_call: arena::Handle<checked_trees::FlowCallFact>,
        argument: ExpressionHandle,
        parameter: &typed_trees::signature::StateParameter,
        value: checked_trees::CheckedStructuralValueHandle,
        argument_ordinal: u32,
        temporary_cleanups: &mut Vec<checked_trees::CheckedScalarComputationTemporaryCleanup>,
    ) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
        use typed_trees::types::TypeReferenceNode;
        // A view lends its completed descriptor, not an
        // unsized owned referent. Use the same carrier
        // classification as ordinary calls and arrivals.
        let descriptor = validation::slice_view_shell(self.program, parameter.type_reference, &[]);
        let (access, referent) = match self
            .program
            .type_reference_table
            .type_reference(parameter.type_reference)
        {
            TypeReferenceNode::Reference { access, .. } if descriptor.is_some() => {
                ((*access).into(), parameter.type_reference)
            }
            TypeReferenceNode::Reference {
                access: language_semantics::ReferenceAccess::Shared,
                ..
            } => (
                checked_trees::CheckedStructuralAccess::SharedBorrow,
                crate::semantic::calls::shared_argument_referent(
                    self.program,
                    argument,
                    parameter.type_reference,
                )?
                .1,
            ),
            TypeReferenceNode::Reference { .. } => return None,
            _ => (
                checked_trees::CheckedStructuralAccess::Owned,
                parameter.type_reference,
            ),
        };
        let (expression, owner_type, path) = self.retained_argument_owner(value, referent)?;
        // The owned-value contract cannot manufacture a
        // reference. A descriptor instead retains the
        // formation/call's already admitted loan custody.
        if descriptor.is_none() {
            validation::structural_result_contract(self.program, referent).ok()?;
        }
        let argument_plan = checked_trees::CheckedUnitStructuralArgumentPlan {
            source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                expression,
            },
            path,
            type_identity: validation::structural_type_identity(self.program, referent, &[], &[]),
            access,
        };
        let mut discards = None;
        if access != checked_trees::CheckedStructuralAccess::Owned {
            // Retention only supplies storage. The permission pass owns
            // the loan's exact consumer and dying continuation; carry its
            // settlement into the scalar call instead of inferring a drop
            // later from the reference formal or constructor spelling.
            // Descriptor formation already rejoins its
            // backing loan at this invocation. Only an
            // anonymous referent home needs a separate
            // temporary-owner loan settlement.
            if descriptor.is_none() {
                crate::execution::terminal_unit::calls::validate_shared_temporary(
                    self.program,
                    self.flow,
                    self.machine,
                    self.state,
                    self.flow.control.calls.get(source_call),
                    expression,
                    owner_type,
                )?;
            }
            match self.program.type_multiplicity(owner_type) {
                language_semantics::Multiplicity::Affine
                | language_semantics::Multiplicity::Linear => {
                    discards = Some(vec![checked_trees::CheckedNormalCleanupAction::Discard(
                        checked_trees::CheckedUnitPartialAffineDiscardPlan {
                            source: argument_plan.source.clone(),
                            path: Vec::new(),
                            type_identity: self.program.normalized_type_identity(owner_type),
                            case_conditions: Default::default(),
                        },
                    )]);
                }
                language_semantics::Multiplicity::Unrestricted => {}
            }
        } else if !argument_plan.path.is_empty() {
            discards = Some(
                crate::execution::terminal_unit::cleanup::projected_temporary_cleanup(
                    self.program,
                    self.flow,
                    self.machine,
                    self.state,
                    self.flow.control.calls.get(source_call),
                    owner_type,
                    &argument_plan,
                )?,
            );
        }
        if let Some(discards) = discards {
            temporary_cleanups.push(checked_trees::CheckedScalarComputationTemporaryCleanup {
                argument_ordinal: Some(argument_ordinal),
                actions: self.plans.temporary_cleanup_actions.insert_many(discards),
            });
        }
        // The retained root owns the complete carrier; the
        // argument's path selects the type matched to this
        // formal. Qualifications are not type rows.
        Some(argument_plan)
    }
}

/// One computed call's operand lanes, retained in the computation arenas.
pub(super) struct ComputedCallOperands {
    pub(super) arguments: arena::HandleSpan<checked_trees::CheckedScalarComputationHandle>,
    pub(super) structural_arguments:
        arena::HandleSpan<checked_trees::CheckedScalarComputationStructuralArgument>,
    pub(super) temporary_cleanups:
        arena::HandleSpan<checked_trees::CheckedScalarComputationTemporaryCleanup>,
    pub(super) claim_transfers: arena::HandleSpan<checked_trees::CheckedUnitClaimTransferPlan>,
}
