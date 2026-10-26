//! Expand checked expression evaluation into private typed blocks.
//!
//! Source values remain the input prefix. Completed scalar operands extend that
//! prefix; structural producers establish separate places at their authored formal
//! positions. Selection converges with one scalar result, without hoisting the
//! selected path's structural effects. The calls and structural value modules own that
//! mixed schedule and its real structural results. Ordinary and boundary calls
//! share the `calls` scheduler: each scalar operand and structural selector
//! completes in authored order before the exact invocation binds its result.
//! Boundary entry plans retain their own contract and foreign ABI custody.
use super::{
    CheckedScalarExpressionRole, CheckedTrees, IntegerValue, LoweringError,
    PreparedScalarQualifications, QualifiedScalarType, ScalarType, scalar_carriers, unsupported,
    validate_direct_parameter_types,
};
use crate::emission::operation_emission::LoweredScalarBinding;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::expression_preparation::bindings as storage;
use crate::expression_preparation::computation_graph::reachable_nodes;
use crate::scalar_lowering::branch_states::lower_scalar_call;
use crate::scalar_lowering::branch_states::prepared::{
    LoweredScalarBranchState, LoweredScalarBranchTerminator, LoweredScalarEffect,
};
use arena::Handle;
use checked_trees::{CheckedScalarComputation, CheckedScalarComputationKind};

pub(crate) mod arrays;
mod byte_comparisons;
mod call_signature;
pub(crate) mod call_views;
mod calls;
pub(crate) mod cases;
pub(crate) mod comparisons;
mod dispatch;
pub(crate) mod fields;
pub(crate) mod literals;
pub(crate) mod observations;
mod operand_scope;
pub(crate) mod operand_types;
pub(crate) mod produced_values;
mod returns;
mod selection;
mod structural_arguments;
pub(crate) mod subslices;
mod value_types;
pub(crate) use value_types::computation_value_type;

type Computation = Handle<CheckedScalarComputation>;

enum Argument {
    Value(LoweredDirectExpression),
    Computation(Computation),
}

struct Site<'a> {
    state: symbols::SymbolHandle,
    statement: u32,
    bindings: &'a storage::ScalarBindings,
}

#[derive(Clone)]
enum EstablishedValue {
    Produced {
        root: checked_trees::CheckedStructuralValueHandle,
    },
    Copied {
        expression: typed_trees::expression::ExpressionHandle,
        place: semantic_vocabulary::PlaceId,
        structural_type: semantic_vocabulary::StructuralTypeId,
        source: crate::emission::operation_emission::calls::PreparedStructuralArgument,
    },
}

/// Prepared scalar control retains value dependencies, not established storage.
/// Only the common producer publishes physical bindings during emission.
pub(crate) struct PreparedExpansion {
    pub(crate) states: Vec<LoweredScalarBranchState>,
    pub(crate) claim_edges: Vec<PreparedClaimEdge>,
}

pub(crate) struct PreparedClaimEdge {
    pub(crate) state_position: usize,
    pub(crate) destinations: Vec<language_semantics::PermissionClaimIdentity>,
    pub(crate) route: Vec<crate::emission::claim_bindings::selections::SelectedDecision>,
}

pub(crate) struct Expansion<'a> {
    /// Private places are allocated when the actual prepared path is known,
    /// from the enclosing emission's one place namespace.
    next_place: &'a mut u64,
    checked: &'a CheckedTrees,
    structural_types: &'a [terminal_psi::StructuralTypeDeclaration],
    qualifications: &'a PreparedScalarQualifications,
    /// Authored owner of the current source state. Fused graphs retain each
    /// state's original computation and call identities under that owner.
    machine: symbols::SymbolHandle,
    base: usize,
    /// Erased-proof roster of the enclosing state being expanded: every state
    /// `push`ed while this is set inherits it, so `Formal` positions in its
    /// edge terms stay in scope on the emitted block.
    erased_proof_formals: Vec<terminal_psi::ErasedProofFormal>,
    states: Vec<LoweredScalarBranchState>,
    claim_edges: Vec<PreparedClaimEdge>,
    calls: Vec<SourceCallCoordinate>,
    produced_values: Vec<produced_values::Slot>,
    call_views: Vec<call_views::Slot>,
    subslices: Vec<subslices::Slot>,
    observations: Vec<observations::Slot>,
    source_carriers: Vec<(
        semantic_vocabulary::PlaceId,
        terminal_psi::StructuralMultiplicity,
    )>,
    literals: Vec<literals::Slot>,
    cases: Vec<cases::Slot>,
    fields: &'a [fields::Binding],
    /// Computations already carried in the current ordered input prefix.
    completed: Vec<(Computation, usize)>,
    /// Earlier structural operands on the selected evaluation path. A later
    /// projection observes their established homes without replaying producers
    /// or acquiring ownership of the enclosing call's temporary.
    established_values: Vec<EstablishedValue>,
}

impl<'a> Expansion<'a> {
    pub(crate) fn has_established_value(
        &self,
        value: checked_trees::CheckedStructuralValueHandle,
    ) -> bool {
        self.established_values.iter().any(|established| {
            matches!(established, EstablishedValue::Produced { root } if *root == value)
        })
    }

    pub(crate) fn new(
        checked: &'a CheckedTrees,
        qualifications: &'a PreparedScalarQualifications,
        machine: symbols::SymbolHandle,
        base: usize,
        next_place: &'a mut u64,
    ) -> Self {
        Self {
            next_place,
            checked,
            structural_types: &[],
            qualifications,
            machine,
            base,
            erased_proof_formals: Vec::new(),
            states: Vec::new(),
            claim_edges: Vec::new(),
            calls: Vec::new(),
            produced_values: Vec::new(),
            call_views: Vec::new(),
            subslices: Vec::new(),
            observations: Vec::new(),
            source_carriers: Vec::new(),
            literals: Vec::new(),
            cases: Vec::new(),
            fields: &[],
            completed: Vec::new(),
            established_values: Vec::new(),
        }
    }

    pub(crate) fn with_structural_types(
        mut self,
        types: &'a [terminal_psi::StructuralTypeDeclaration],
    ) -> Self {
        self.structural_types = types;
        self
    }

    /// Enter the erased-proof scope of one enclosing checked state before
    /// lowering its computations.
    pub(crate) fn enter_proof_scope(
        mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        roster: &[checked_trees::CheckedErasedProofParameterPlan],
    ) -> Self {
        self.erased_proof_formals =
            crate::scalar_lowering::contracts::erased_proof_formal_declarations(
                identities,
                self.checked,
                roster,
            );
        self
    }

    pub(crate) fn finish(self) -> PreparedExpansion {
        PreparedExpansion {
            states: self.states,
            claim_edges: self.claim_edges,
        }
    }

    pub(crate) fn with_fields(mut self, fields: &'a [fields::Binding]) -> Self {
        self.fields = fields;
        self
    }

    pub(crate) fn with_produced_values(
        mut self,
        produced_values: &[produced_values::Slot],
    ) -> Self {
        self.produced_values = produced_values.to_vec();
        self
    }

    /// Seed only values already emitted in this operand frame. Reserved slots
    /// are not completion evidence, and a missing captured home must not cause
    /// a copyable producer's effects to run again in a fresh scalar expansion.
    pub(crate) fn with_completed_values(
        mut self,
        completed: impl IntoIterator<Item = checked_trees::CheckedStructuralValueHandle>,
    ) -> Self {
        self.established_values.extend(
            completed
                .into_iter()
                .map(|root| EstablishedValue::Produced { root }),
        );
        self
    }

    pub(crate) fn with_source_carriers(
        mut self,
        carriers: Vec<(
            semantic_vocabulary::PlaceId,
            terminal_psi::StructuralMultiplicity,
        )>,
    ) -> Self {
        self.source_carriers = carriers;
        self
    }

    pub(crate) fn with_observations(mut self, observations: &[observations::Slot]) -> Self {
        self.observations = observations.to_vec();
        self
    }

    pub(crate) fn with_call_views(mut self, call_views: &[call_views::Slot]) -> Self {
        self.call_views = call_views.to_vec();
        self
    }

    pub(crate) fn with_subslices(mut self, subslices: &[subslices::Slot]) -> Self {
        self.subslices = subslices.to_vec();
        self
    }

    pub(crate) fn with_literals(mut self, literals: &[literals::Slot]) -> Self {
        self.literals = literals.to_vec();
        self
    }

    pub(crate) fn with_cases(mut self, cases: &[cases::Slot]) -> Self {
        self.cases = cases.to_vec();
        self
    }

    /// Retain the caller prefix while completing each operand in source order.
    /// The target receives that prefix followed by the dense scalar arguments.
    /// A slice retains its original dense scalar ordinal for source custody.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn call_arguments(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        state: symbols::SymbolHandle,
        coordinate: checked_trees::CheckedUnitCallCoordinate,
        arguments: &[checked_trees::CheckedCallScalarArgument],
        argument_ordinal_start: usize,
        bindings: &storage::ScalarBindings,
        source_types: &[QualifiedScalarType],
        target: usize,
    ) -> Result<usize, LoweringError> {
        let site = Site {
            state,
            statement: coordinate.statement_index,
            bindings,
        };
        let mut operands = Vec::with_capacity(arguments.len());
        for (ordinal, argument) in arguments.iter().enumerate() {
            let ordinal =
                argument_ordinal_start
                    .checked_add(ordinal)
                    .ok_or(LoweringError::Unsupported(
                        "scalar call argument ordinal overflows",
                    ))?;
            let _ = u32::try_from(ordinal).map_err(|_| {
                LoweringError::Unsupported("scalar call argument ordinal exceeds u32")
            })?;
            operands.push(match argument {
                checked_trees::CheckedCallScalarArgument::Pure(expression) => {
                    Argument::Value(bindings.expression(
                        identities,
                        &self.checked.facts.proven_scalar_indexes,
                        expression,
                    )?)
                }
                checked_trees::CheckedCallScalarArgument::Computation(root) => {
                    Argument::Computation(*root)
                }
            });
        }
        self.sequence(
            identities,
            &operands,
            source_types,
            target,
            &site,
            &mut Vec::new(),
        )
    }

    /// Preserve the input prefix while a pure operand's selective expression
    /// completes through the ordinary scalar block emitter.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn retained_pure_value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        state: symbols::SymbolHandle,
        statement: u32,
        role: CheckedScalarExpressionRole,
        bindings: &storage::ScalarBindings,
        source_types: &[QualifiedScalarType],
        result_type: QualifiedScalarType,
        target: usize,
    ) -> Result<usize, LoweringError> {
        let expression =
            bindings.expression_at(identities, self.checked, state, statement, role)?;
        if expression.value_type(source_types)? != result_type {
            return unsupported("retained scalar operand disagrees with its destination type");
        }
        self.argument(
            identities,
            &Argument::Value(expression),
            source_types,
            target,
            &Site {
                state,
                statement,
                bindings,
            },
            &mut Vec::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn retained_value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        state: symbols::SymbolHandle,
        statement: u32,
        role: CheckedScalarExpressionRole,
        bindings: &storage::ScalarBindings,
        source_types: &[QualifiedScalarType],
        result_type: QualifiedScalarType,
        target: usize,
    ) -> Result<usize, LoweringError> {
        let mut roots = self
            .checked
            .facts
            .values
            .scalar_computations
            .roots_for_statement_role(state, statement, role);
        let root = roots.next().ok_or(LoweringError::Unsupported(
            "scalar retained value has no checked computation root",
        ))?;
        if roots.next().is_some() {
            return unsupported("scalar computation root custody is duplicated or mismatched");
        }
        self.retained_root(
            identities,
            root,
            bindings,
            source_types,
            result_type,
            target,
        )
    }

    /// Complete one retained root through the same scheduling owner as a source
    /// destination. The root keeps its exact declaration frame; callers may
    /// supply a captured formal namespace without replaying source operands.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn retained_root(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        root: &checked_trees::CheckedScalarComputationRoot,
        bindings: &storage::ScalarBindings,
        source_types: &[QualifiedScalarType],
        result_type: QualifiedScalarType,
        target: usize,
    ) -> Result<usize, LoweringError> {
        let argument = self.retained_root_argument(identities, root, bindings)?;
        let site = Site {
            state: root.state,
            statement: root.statement_ordinal,
            bindings,
        };
        if self.argument_type(identities, &argument, &site, source_types)? != result_type {
            return unsupported("scalar computation result disagrees with its destination");
        }
        // Unlike a state transfer, the following statements retain every prior
        // completed value, including immutable snapshots of overwritten storage.
        self.argument(
            identities,
            &argument,
            source_types,
            target,
            &site,
            &mut Vec::new(),
        )
    }

    fn retained_root_argument(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        root: &checked_trees::CheckedScalarComputationRoot,
        bindings: &storage::ScalarBindings,
    ) -> Result<Argument, LoweringError> {
        if root.machine != self.machine || !root.root.is_valid() {
            return unsupported("scalar computation root custody is duplicated or mismatched");
        }
        let site = Site {
            state: root.state,
            statement: root.statement_ordinal,
            bindings,
        };
        // Each retained call node rejoins its exact FlowCall occurrence in
        // calls::call. The proof invocation subsequently joins that emitted
        // operation by the same state/statement/ordinal, including nested
        // scalar calls. A different proof-bearing call in this machine cannot
        // invalidate this operand's operation custody.
        if let CheckedScalarExpressionRole::LocalInitializer { binding_ordinal } = root.role {
            calls::consume_pure_call_arguments(
                identities,
                self.checked,
                site.bindings,
                root.state,
                root.statement_ordinal,
                binding_ordinal,
                root.root,
            )?;
        }
        Ok(Argument::Computation(root.root))
    }

    /// One no-code edge that forwards `input_types` unchanged and hands its
    /// final value, carried at `value_type`, to `target` as a fresh value of
    /// the type `target` declares there. Emission records the edge as an
    /// introduction or erasure of membership and refuses anything else.
    pub(crate) fn qualification_edge(
        &mut self,
        input_types: &[QualifiedScalarType],
        value_type: QualifiedScalarType,
        target: usize,
    ) -> usize {
        let mut value_types = input_types.to_vec();
        value_types.push(value_type);
        self.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            parameter_types: value_types.clone(),
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: Vec::new(),
            structural_effects: Vec::new(),
            terminator: LoweredScalarBranchTerminator::Qualify {
                target,
                arguments: parameters(&value_types),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
            },
        })
    }

    pub(crate) fn push(&mut self, mut state: LoweredScalarBranchState) -> usize {
        let index = self.base + self.states.len();
        state.erased_proof_formals = self.erased_proof_formals.clone();
        self.states.push(state);
        index
    }

    fn argument_type(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        argument: &Argument,
        site: &Site<'_>,
        source_types: &[QualifiedScalarType],
    ) -> Result<QualifiedScalarType, LoweringError> {
        match argument {
            Argument::Value(value) => value.value_type(source_types),
            Argument::Computation(handle) => computation_value_type(
                identities,
                self.checked,
                self.qualifications,
                *handle,
                site.bindings,
                source_types,
            ),
        }
    }

    fn sequence(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        arguments: &[Argument],
        input_types: &[QualifiedScalarType],
        target: usize,
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        let mut prefixes = vec![input_types.to_vec()];
        for argument in arguments {
            let mut next = prefixes.last().expect("input prefix").clone();
            next.push(self.argument_type(identities, argument, site, input_types)?);
            prefixes.push(next);
        }
        let mut continuation = target;
        for (index, argument) in arguments.iter().enumerate().rev() {
            let saved = self.completed.len();
            self.completed
                .extend(
                    arguments[..index]
                        .iter()
                        .enumerate()
                        .filter_map(|(earlier, argument)| match argument {
                            Argument::Computation(computation) => {
                                Some((*computation, prefixes[earlier].len()))
                            }
                            Argument::Value(_) => None,
                        }),
                );
            continuation = self.argument(
                identities,
                argument,
                &prefixes[index],
                continuation,
                site,
                active,
            )?;
            self.completed.truncate(saved);
        }
        Ok(continuation)
    }

    /// One state that evaluates an already-lowered direct expression over the
    /// whole input prefix and forwards it appended to `target`. A short-circuit
    /// Boolean among them is staged through its own decision blocks at
    /// emission, exactly as an expanded computation's binding is.
    pub(crate) fn direct_value(
        &mut self,
        input_types: &[QualifiedScalarType],
        target: usize,
        expression: LoweredDirectExpression,
    ) -> Result<usize, LoweringError> {
        validate_direct_parameter_types(&expression, &scalar_carriers(input_types))?;
        Ok(self.binding(
            input_types,
            input_types.len(),
            target,
            LoweredScalarBinding::Expression(expression),
        ))
    }

    fn binding(
        &mut self,
        input_types: &[QualifiedScalarType],
        retained: usize,
        target: usize,
        binding: LoweredScalarBinding,
    ) -> usize {
        let mut arguments = parameters(&input_types[..retained]);
        arguments.push(parameter(input_types.len(), binding.scalar_type().into()));
        self.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            structural_effects: Vec::new(),
            parameter_types: input_types.to_vec(),
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: vec![binding],
            terminator: LoweredScalarBranchTerminator::jump(target, arguments),
        })
    }

    fn argument(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        argument: &Argument,
        input_types: &[QualifiedScalarType],
        target: usize,
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        let Argument::Computation(handle) = argument else {
            let Argument::Value(expression) = argument else {
                unreachable!()
            };
            validate_direct_parameter_types(expression, &scalar_carriers(input_types))?;
            return Ok(self.binding(
                input_types,
                input_types.len(),
                target,
                LoweredScalarBinding::Expression(expression.clone()),
            ));
        };
        if active.contains(handle) {
            return unsupported("scalar computation contains a cycle");
        }
        let result_type = self.argument_type(identities, argument, site, input_types)?;
        if let Some((_, position)) = self.completed.iter().find(|(node, _)| node == handle) {
            if input_types.get(*position) != Some(&result_type) {
                return unsupported("completed computation disagrees with its carried value");
            }
            return Ok(self.binding(
                input_types,
                input_types.len(),
                target,
                LoweredScalarBinding::Expression(parameter(*position, result_type)),
            ));
        }
        active.push(*handle);
        let plans = &self.checked.facts.values.scalar_computations;
        let node = plans.nodes.get(*handle).clone();
        let completion_expression = node.completion_expression(&self.checked.facts.flow.control);
        let completion = self
            .checked
            .facts
            .flow
            .terminal_unit_effects
            .cleanup_control
            .value_completion(
                self.machine,
                site.state,
                site.statement,
                completion_expression,
            )
            .cloned();
        let completion_state = completion.as_ref().map(|_| {
            let mut completed_types = input_types.to_vec();
            completed_types.push(result_type);
            self.push(LoweredScalarBranchState::jumping(
                target,
                Vec::new(),
                completed_types.clone(),
                &completed_types,
                vec![LoweredScalarEffect::CompleteValueObservation {
                    machine: self.machine,
                    state: site.state,
                    statement: site.statement,
                    expression: completion_expression,
                }],
            ))
        });
        let target = completion_state.unwrap_or(target);
        let entry = match node.kind {
            CheckedScalarComputationKind::StorageAddress { subject, .. } => {
                let (_, state) = crate::declarations::state_declaration(self.checked, site.state)?;
                let source = site
                    .bindings
                    .borrowed_structural_home(&subject, self.checked.state_parameters(state))?;
                let root_type = site.bindings.structural_argument_type(&source).ok_or(
                    LoweringError::Unsupported("address query lost its referent carrier"),
                )?;
                let mut path = source.path.into_iter().map(crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment::Static).collect::<Vec<_>>();
                path.extend(crate::expression_preparation::bindings::structural_paths::prepare_structural_argument_path(
                    identities, self.checked, &subject, root_type, self.structural_types)?);
                self.binding(
                    input_types,
                    input_types.len(),
                    target,
                    LoweredScalarBinding::StorageAddress(
                        crate::emission::operation_emission::calls::PreparedStructuralArgument {
                            source: source.place.into(),
                            root_type: Some(root_type),
                            access: source.access,
                            path,
                            selector_positions: Vec::new(),
                        },
                        result_type.scalar_type,
                    ),
                )
            }

            CheckedScalarComputationKind::ByteSequenceLiteralComparison {
                literal,
                maximum_length,
                negated,
                subject,
                ..
            } => self.byte_literal_comparison(
                identities,
                &literal,
                maximum_length,
                negated,
                &subject,
                input_types,
                target,
                site,
                active,
            )?,
            CheckedScalarComputationKind::ByteSequencePredicate {
                predicate, subject, ..
            } => self.byte_predicate(
                identities,
                predicate,
                &subject,
                input_types,
                target,
                site,
                active,
            )?,
            CheckedScalarComputationKind::StructuralField {
                atomic_ordering,
                receiver_expression,
                receiver_value,
                subject,
                field,
                ..
            } => self.structural_field(
                *handle,
                receiver_expression,
                receiver_value,
                &subject,
                field,
                atomic_ordering,
                node.primitive_type,
                node.normal_cleanup_actions,
                input_types,
                target,
                site,
            )?,
            CheckedScalarComputationKind::CaseMembership {
                subject,
                case,
                receiver_value,
                ..
            } => self.case_membership(
                identities,
                *handle,
                &subject,
                receiver_value,
                case,
                input_types,
                target,
                site,
                active,
            )?,
            CheckedScalarComputationKind::SelectedComparison {
                operator_use,
                left,
                right,
            } => {
                let operand_type = self.argument_type(
                    identities,
                    &Argument::Computation(left),
                    site,
                    input_types,
                )?;
                let mut tested_types = input_types.to_vec();
                tested_types.extend([operand_type, operand_type]);
                let binding = self.comparison_binding(
                    operator_use,
                    parameter(input_types.len(), operand_type),
                    parameter(input_types.len() + 1, operand_type),
                )?;
                let comparison = self.binding(&tested_types, input_types.len(), target, binding);
                self.sequence(
                    identities,
                    &[Argument::Computation(left), Argument::Computation(right)],
                    input_types,
                    comparison,
                    site,
                    active,
                )?
            }
            CheckedScalarComputationKind::Qualification { operand, .. } => {
                let operand = Argument::Computation(operand);
                let operand_type = self.argument_type(identities, &operand, site, input_types)?;
                if operand_type == result_type {
                    self.argument(identities, &operand, input_types, target, site, active)?
                } else {
                    // The target was prepared with the full result type. Only
                    // this edge establishes it, leaving the operand unchanged.
                    let qualify = self.qualification_edge(input_types, operand_type, target);
                    self.argument(identities, &operand, input_types, qualify, site, active)?
                }
            }
            CheckedScalarComputationKind::AuthoredCast {
                operand,
                formation_decided,
                ..
            } => {
                // The operand completes at its own carrier, then the authored
                // exact conversion applies once. Result qualifications remain
                // on the receiving edge and cannot alter this payload.
                let operand = Argument::Computation(operand);
                let operand_type = self.argument_type(identities, &operand, site, input_types)?;
                // Conversion bindings bind the operand's bare carrier: an
                // operand that still carries domains erases them on a
                // qualification edge before the conversion state.
                let bound_operand_type = QualifiedScalarType::from(operand_type.scalar_type);
                let mut operand_types = input_types.to_vec();
                operand_types.push(bound_operand_type);
                let operand_expression = parameter(input_types.len(), bound_operand_type);
                let conversion =
                    crate::expression_preparation::integer_coercion::coerce_integer_carrier(
                        operand_expression,
                        result_type.scalar_type,
                        formation_decided,
                    )?;
                let conversion_target = if result_type.qualifications.is_empty() {
                    target
                } else {
                    // Carrier conversion binds the unchanged payload first.
                    // The authored qualification has its own edge, just as
                    // a representation-identical Qualification node does.
                    self.qualification_edge(input_types, result_type.scalar_type.into(), target)
                };
                let bound = self.binding(
                    &operand_types,
                    input_types.len(),
                    conversion_target,
                    LoweredScalarBinding::Expression(conversion),
                );
                let bound = if operand_type.qualifications.is_empty() {
                    bound
                } else {
                    self.qualification_edge(input_types, operand_type, bound)
                };
                self.argument(identities, &operand, input_types, bound, site, active)?
            }
            CheckedScalarComputationKind::Dispatch {
                source_expression,
                subject,
                arms,
            } => self.dispatch(
                identities,
                source_expression,
                subject,
                arms,
                result_type,
                input_types,
                target,
                site,
                active,
            )?,
            CheckedScalarComputationKind::LayoutObservation {
                kind,
                operand,
                carrier_identity,
                ..
            } => {
                if !self
                    .checked
                    .type_reference_table
                    .contains_type_reference(operand)
                {
                    return unsupported("layout observation lost its exact checked type operand");
                }
                let carrier = if matches!(
                    self.checked.type_reference_table.type_reference(operand),
                    typed_trees::types::TypeReferenceNode::Unit
                ) {
                    terminal_psi::LayoutObservationCarrier::Unit
                } else if let Some(primitive) = self.checked.primitive_type_reference(operand) {
                    terminal_psi::LayoutObservationCarrier::Scalar(super::terminal_scalar_type(
                        primitive,
                    )?)
                } else {
                    let declaration =
                        crate::terminal_identities::structural_declaration_by_identity(
                            self.structural_types,
                            identities,
                            self.checked,
                            carrier_identity,
                            "layout observation lost its checked carrier in the structural catalog",
                        )?;
                    terminal_psi::LayoutObservationCarrier::Structural(declaration.id)
                };
                let kind = match kind {
                    language_core::data_layout_queries::DataLayoutQueryKind::Size => {
                        terminal_psi::LayoutObservationKind::Size
                    }
                    language_core::data_layout_queries::DataLayoutQueryKind::Alignment => {
                        terminal_psi::LayoutObservationKind::Alignment
                    }
                };
                self.binding(
                    input_types,
                    input_types.len(),
                    target,
                    LoweredScalarBinding::LayoutObservation {
                        kind,
                        carrier,
                        scalar_type: result_type.scalar_type,
                    },
                )
            }
            CheckedScalarComputationKind::RetainedConstant { expression, .. } => {
                let Some(literal) = self.checked.landed_integer_constant(expression) else {
                    return unsupported("retained scalar constant is not closed for the target");
                };
                let expression = site.bindings.expression(
                    identities,
                    &self.checked.facts.proven_scalar_indexes,
                    &checked_trees::CheckedScalarExpression::IntegerLiteral {
                        literal: literal.clone(),
                    },
                )?;
                self.binding(
                    input_types,
                    input_types.len(),
                    target,
                    LoweredScalarBinding::Expression(expression),
                )
            }
            CheckedScalarComputationKind::Value(expression) => {
                let expression = site.bindings.expression(
                    identities,
                    &self.checked.facts.proven_scalar_indexes,
                    &expression,
                )?;
                if expression.value_type(input_types)? != result_type {
                    return unsupported("scalar computation value carrier disagrees");
                }
                validate_direct_parameter_types(&expression, &scalar_carriers(input_types))?;
                self.binding(
                    input_types,
                    input_types.len(),
                    target,
                    LoweredScalarBinding::Expression(expression),
                )
            }
            CheckedScalarComputationKind::Select {
                source_expression,
                condition,
                when_true,
                when_false,
                ..
            } => self.selection(
                identities,
                source_expression,
                condition,
                when_true,
                when_false,
                result_type,
                input_types,
                target,
                site,
                active,
            )?,
            CheckedScalarComputationKind::BooleanToInteger { operand, .. } => {
                // The conversion is exactly one conditional landing of 0 or 1
                // in the destination carrier; the authored cast keeps source
                // custody while the branch shape reuses ordinary selection.
                let operand = Argument::Computation(operand);
                if self.argument_type(identities, &operand, site, input_types)?
                    != ScalarType::Boolean.into()
                {
                    return unsupported(
                        "Boolean-to-integer conversion operand is not a scalar Boolean",
                    );
                }
                let Some(integer_type) = result_type.scalar_type.as_integer() else {
                    return unsupported(
                        "Boolean-to-integer conversion result is not an integer carrier",
                    );
                };
                let literal = |value: u128| {
                    LoweredScalarBinding::Expression(LoweredDirectExpression::IntegerLiteral {
                        value: match integer_type.sign() {
                            semantic_vocabulary::IntegerSign::Signed => {
                                IntegerValue::Signed(value as i128)
                            }
                            semantic_vocabulary::IntegerSign::Unsigned => {
                                IntegerValue::Unsigned(value)
                            }
                        },
                        scalar_type: result_type.scalar_type,
                    })
                };
                let when_true_target =
                    self.binding(input_types, input_types.len(), target, literal(1));
                let when_false_target =
                    self.binding(input_types, input_types.len(), target, literal(0));
                let mut condition_types = input_types.to_vec();
                condition_types.push(ScalarType::Boolean.into());
                let dispatch = self.push(LoweredScalarBranchState {
                    structural_parameters: Vec::new(),
                    structural_effects: Vec::new(),
                    parameter_types: condition_types,
                    erased_formal_types: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    bindings: Vec::new(),
                    terminator: LoweredScalarBranchTerminator::Conditional {
                        condition: LoweredBooleanReturnExpression::Parameter {
                            position: input_types.len(),
                        },
                        when_true_target,
                        when_true_arguments: parameters(input_types),
                        when_true_erased_arguments: Vec::new(),
                        when_true_erased_proof_arguments: Vec::new(),
                        when_false_target,
                        when_false_arguments: parameters(input_types),
                        when_false_erased_arguments: Vec::new(),
                        when_false_erased_proof_arguments: Vec::new(),
                    },
                });
                self.argument(identities, &operand, input_types, dispatch, site, active)?
            }
            CheckedScalarComputationKind::Call {
                source_call,
                callee,
                call_ordinal,
                arguments,
                structural_arguments,
                temporary_cleanups,
                claim_transfers,
            } => self.call(
                identities,
                source_call,
                &callee,
                call_ordinal,
                arguments,
                structural_arguments,
                temporary_cleanups,
                claim_transfers,
                result_type,
                input_types,
                target,
                site,
                active,
            )?,
            CheckedScalarComputationKind::StorageObservation {
                root_type,
                expression,
                operands,
                collections,
                ..
            } => self.storage_observation(
                identities,
                *handle,
                root_type,
                &expression,
                operands,
                collections,
                input_types,
                result_type,
                target,
                site,
                active,
            )?,
            CheckedScalarComputationKind::WithOperands {
                operands, operand, ..
            } => self.operand_scope(
                identities,
                operands,
                operand,
                input_types,
                target,
                site,
                active,
            )?,
            CheckedScalarComputationKind::Apply {
                expression,
                operands,
                ..
            } => {
                let operands = plans
                    .operands
                    .span(operands)
                    .ok_or(LoweringError::Unsupported(
                        "scalar computation application has an invalid operand span",
                    ))?
                    .iter()
                    .copied()
                    .map(Argument::Computation)
                    .collect::<Vec<_>>();
                let mut operand_types = input_types.to_vec();
                for operand in &operands {
                    operand_types.push(self.argument_type(
                        identities,
                        operand,
                        site,
                        input_types,
                    )?);
                }
                let expression = site
                    .bindings
                    .clone()
                    .with_computation_operands(
                        input_types.len(),
                        operand_types[input_types.len()..]
                            .iter()
                            .map(|value| value.scalar_type),
                    )
                    .expression(
                        identities,
                        &self.checked.facts.proven_scalar_indexes,
                        &expression,
                    )?;
                if expression.value_type(&operand_types)? != result_type {
                    return unsupported("scalar computation application carrier disagrees");
                }
                validate_direct_parameter_types(&expression, &scalar_carriers(&operand_types))?;
                let apply = self.binding(
                    &operand_types,
                    input_types.len(),
                    target,
                    LoweredScalarBinding::Expression(expression),
                );
                self.sequence(identities, &operands, input_types, apply, site, active)?
            }
        };
        active.pop();
        Ok(entry)
    }
}

fn parameter(position: usize, value_type: QualifiedScalarType) -> LoweredDirectExpression {
    LoweredDirectExpression::Parameter {
        position,
        scalar_type: value_type.scalar_type,
    }
}

pub(crate) fn parameters(types: &[QualifiedScalarType]) -> Vec<LoweredDirectExpression> {
    types
        .iter()
        .copied()
        .enumerate()
        .map(|(position, scalar_type)| parameter(position, scalar_type))
        .collect()
}

/// Every scalar-computation node reachable from `machine`'s roots.
pub(super) fn machine_computation_handles(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<checked_trees::CheckedScalarComputationHandle>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans.roots_for_machine(machine);
    reachable_nodes(checked, &roots)
}
