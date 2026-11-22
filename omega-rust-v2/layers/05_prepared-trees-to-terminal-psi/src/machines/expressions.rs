use super::body::{MachineBodyValues, check_container, next_id, site_at, unsupported_node};
use super::branches::{BlockFlow, BooleanArm, TestedDispatch, tested_dispatch, value_patterns};
use crate::lowering::applied_tree::LoweringTree;
use crate::{
    BlockId, BooleanPrimitive, CallArgument, CallTarget, LoweringError, LoweringTypes, MachineBody, OperationId, OperationKind, OperationResult, Place,
    PlaceId, PlaceOrigin, PsiOperation, ScalarLiteral, TypeId, ValueId, ValueOrigin,
};
use num_bigint::BigInt;
use omega_prepared::{PreparedTrees, PreparedValue};
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind;
use omega_typed::{NodeSemantics, Site, TypedSignature, ValueType};

pub(super) trait EmitOperation {
    fn push_operation(
        &mut self,
        output: OperationOutput,
        kind: OperationKind,
        site: Site,
    ) -> OperationResult;
}
impl EmitOperation for MachineBody {
    fn push_operation(
        &mut self,
        output: OperationOutput,
        kind: OperationKind,
        site: Site,
    ) -> OperationResult {
        let id = OperationId(next_id(self.operations.len()));
        let result = match output {
            OperationOutput::Unit => OperationResult::Unit,
            OperationOutput::Scalar(ty) => OperationResult::Scalar(self.push_value(ty, ValueOrigin::Operation(id))),
            OperationOutput::Structural(ty) => {
                let place = PlaceId(next_id(self.places.len()));
                self.places.push(Place {
                    ty,
                    origin: PlaceOrigin::Operation(id),
                });
                OperationResult::Structural(place)
            }
        };
        self.operations.push(PsiOperation { id, result, kind });
        self.diagnostics.operations.push(site);
        result
    }
}

pub(crate) struct Expressions<'a> {
    pub(super) machine: crate::MachineId,
    pub(super) prepared: &'a PreparedTrees<'a>,
    pub(crate) tree: &'a LoweringTree<'a>,
    pub(crate) signature: &'a TypedSignature,
    pub(super) types: &'a LoweringTypes,
    pub(super) data: &'a [crate::LoweredData],
    pub(super) callees: &'a crate::machines::calls::Callees<'a>,
    pub(super) bindings: Vec<(usize, OperationResult)>,
    /// Selector relations stated by this machine's published entry clauses.
    pub(super) selector_premises: Vec<super::view_loans::SelectorPremise>,
}

#[derive(Clone, Copy)]
pub(super) enum OperationOutput {
    Unit,
    Scalar(TypeId),
    Structural(TypeId),
}

enum PendingOperation {
    Construct(super::construction::Constructor),
    Integer(super::arithmetic::LoweredInteger),
    Float(crate::FloatOperation),
    Boolean(BooleanPrimitive),
    ExactNumericCast,
    ScalarBitRecast {
        access: crate::ReferenceAccess,
        source_referent: crate::ScalarType,
        destination_referent: crate::ScalarType,
    },
    Call {
        callee: CallTarget,
        owed: super::calls::OwedClauses,
        regions: Vec<crate::CallRegion>,
        borrowed: Vec<Option<CallArgument>>,
        pause: crate::PauseEffects,
    },
    FixedArrayRead(Vec<usize>),
    ProjectedScalarRead(Vec<crate::PlaceProjection>),
    CollectionLength(crate::machines::access::CollectionLength),
    ByteSequenceRead {
        selector: usize,
        length_type: TypeId,
    },
    ElementViewLength,
    ElementViewRead {
        selector: usize,
        length_type: TypeId,
    },
    EstablishScalarArray,
    EstablishStructuralArray,
}

enum ExpressionStep<'a> {
    Frozen {
        node: usize,
        value: &'a PreparedValue,
        carrier: &'a ValueType,
    },
    Qualify {
        node: usize,
        target: TypeId,
        change: super::casts::ScalarChange,
    },
    SplitIntegerMatch {
        dispatch: usize,
        subject_carrier: &'a ValueType,
        carrier: &'a ValueType,
        ty: Option<TypeId>,
    },
    SplitMatch {
        subject: usize,
        arms: Vec<BooleanArm>,
        carrier: &'a ValueType,
        ty: Option<TypeId>,
    },
    MatchArms {
        targets: Vec<(BlockId, usize)>,
        carrier: &'a ValueType,
        ty: Option<TypeId>,
    },
    JoinMatch {
        expression: usize,
        remaining: std::vec::IntoIter<(BlockId, usize)>,
        carrier: &'a ValueType,
        join: BlockId,
        ty: Option<TypeId>,
    },
    SplitBoolean {
        node: usize,
        right: usize,
        evaluate_right_when: bool,
        ty: TypeId,
    },
    JoinBoolean {
        right: usize,
        join: BlockId,
        ty: TypeId,
    },
    Visit {
        node: usize,
        carrier: &'a ValueType,
        landing: Option<&'a BigInt>,
    },
    Emit {
        node: usize,
        output: OperationOutput,
        operation: PendingOperation,
        arity: usize,
    },
}

trait CallOperand {
    fn into_call_argument(
        self,
        site: &Site,
    ) -> Result<CallArgument, LoweringError>;
}
impl CallOperand for OperationResult {
    fn into_call_argument(
        self,
        site: &Site,
    ) -> Result<CallArgument, LoweringError> {
        match self {
            Self::Scalar(value) => Ok(CallArgument::Scalar(value)),
            Self::Structural(place) => Ok(CallArgument::CopyWhole(place)),
            Self::Unit => Err(LoweringError::new(site, "unit call cannot supply an argument value")),
        }
    }
}

impl Expressions<'_> {
    pub(crate) fn lower_named_guard(
        &self,
        guard: usize,
        body: &mut MachineBody,
    ) -> Result<ValueId, LoweringError> {
        let resolved = self.tree.resolved();
        let site = site_at(resolved.file, guard);
        let error = || LoweringError::new(&site, "guard has no lowered Boolean value");
        let ty = self.types.id_for(&ValueType::Bool).map_err(|_| error())?;
        let literal = match resolved.symbol(guard) {
            Some(Symbol::Binding { binder_node, .. }) => {
                check_container(self.tree, guard)?;
                return self
                    .bindings
                    .iter()
                    .rev()
                    .find(|(binder, _)| binder == binder_node)
                    .map(|(_, result)| read_local(body, *result, site.clone()))
                    .and_then(|result| match result {
                        OperationResult::Scalar(value) if body.values[value.index()].ty == ty => Some(value),
                        _ => None,
                    })
                    .ok_or_else(error);
            }
            Some(Symbol::Builtin(name)) if name == "false" => {
                check_container(self.tree, guard)?;
                false
            }
            Some(Symbol::Declaration(id)) if id.kind == NodeKind::Const => {
                let mut records = self.tree.semantics_at(guard);
                if !matches!(records.next(),Some(NodeSemantics::Constant(constant)) if constant==id) || records.next().is_some() {
                    return Err(error());
                }
                let Some(PreparedValue::Bool(b)) = self.prepared.constant(id) else {
                    return Err(error());
                };
                *b
            }
            _ => return Err(error()),
        };
        let OperationResult::Scalar(value) = body.push_operation(OperationOutput::Scalar(ty), OperationKind::Constant(ScalarLiteral::Boolean(literal)), site)
        else {
            unreachable!("a Boolean constant produces a scalar")
        };
        Ok(value)
    }

    fn call_value_argument(
        &self,
        value: OperationResult,
        body: &MachineBody,
        site: &Site,
    ) -> Result<CallArgument, LoweringError> {
        if let OperationResult::Structural(place) = value
            && let Some(crate::TypeShape::NominalReference { access, .. } | crate::TypeShape::ScalarReference { access, .. }) =
                self.types.get(body.places[place.index()].ty)
        {
            return Ok(CallArgument::Borrow {
                place: crate::ProjectedPlace {
                    root: place,
                    path: vec![crate::PlaceProjection::Referent],
                },
                access: *access,
            });
        }
        self.value_argument(value, body, site)
    }

    pub(super) fn value_argument(
        &self,
        value: OperationResult,
        body: &MachineBody,
        site: &Site,
    ) -> Result<CallArgument, LoweringError> {
        if let OperationResult::Structural(place) = value
            && super::ownership::owned(body.places[place.index()].ty, self.types, self.data)
        {
            return Ok(CallArgument::MoveWhole(place));
        }
        value.into_call_argument(site)
    }

    /// The operand a match arm supplies to its join parameter. An exclusive
    /// descriptor is affine: the edge transfers the arm's descriptor, so the
    /// join parameter becomes its only name and the arm's source binding is
    /// consumed on that arrival. The checker judges any later use of it.
    fn join_argument(
        &self,
        value: OperationResult,
        carrier: &ValueType,
        body: &MachineBody,
        site: &Site,
    ) -> Result<CallArgument, LoweringError> {
        if crate::lowering::types::is_exclusive_descriptor(carrier) {
            let OperationResult::Structural(place) = value else {
                return Err(LoweringError::new(site, "an exclusive descriptor arm must supply a structural descriptor"));
            };
            return Ok(CallArgument::MoveWhole(place));
        }
        self.value_argument(value, body, site)
    }

    pub(super) fn bind_local(
        &self,
        node: usize,
        carrier: &ValueType,
        body: &mut MachineBody,
        flow: &mut BlockFlow,
    ) -> Result<OperationResult, LoweringError> {
        let site = site_at(self.tree.resolved().file, node);
        if crate::lowering::types::is_exclusive_descriptor(carrier) {
            let value = self.lower(node, carrier, body, flow)?;
            let OperationResult::Structural(source) = value else {
                return Err(LoweringError::new(&site, "an exclusive descriptor requires a structural place"));
            };
            // A named descriptor transfers into its new binding. A fresh result
            // already has its own place, including a match's join parameter.
            if self.bindings.iter().any(|(_, bound)| *bound == value) {
                return Ok(body.push_operation(
                    OperationOutput::Structural(body.places[source.index()].ty),
                    OperationKind::MoveStructural {
                        source: crate::ProjectedPlace { root: source, path: Vec::new() },
                    },
                    site,
                ));
            }
            return Ok(value);
        }
        if crate::lowering::types::place_reference(carrier).is_some() {
            return self.lower(node, carrier, body, flow);
        }
        if !self.types.can_forward_value(carrier) && !crate::lowering::types::contains_nominal_value(carrier) {
            return Err(LoweringError::new(&site, "local binding requires a supported copyable carrier"));
        }
        let value = self.lower(node, carrier, body, flow)?;
        // A local holding a shared descriptor can reuse its existing place.
        // Rebinding the local changes only the binding, never the borrowed bytes.
        if let OperationResult::Structural(source) = value
            && matches!(carrier, ValueType::Array(..))
            && !super::ownership::owned(body.places[source.index()].ty, self.types, self.data)
        {
            let ty = body.places[source.index()].ty;
            return Ok(body.push_operation(
                OperationOutput::Structural(ty),
                OperationKind::StructuralLeafCopy { source, path: Vec::new() },
                site,
            ));
        }
        if let OperationResult::Structural(source) = value
            && carrier.nominal_parts().is_some()
            && self.tree.resolved().file.nodes[self.tree.resolved().file.ungroup(node)].kind == NodeKind::PathExpression
            && !super::ownership::owned(body.places[source.index()].ty, self.types, self.data)
        {
            return Ok(body.push_operation(
                OperationOutput::Structural(body.places[source.index()].ty),
                OperationKind::CopyStructural {
                    source: crate::ProjectedPlace { root: source, path: vec![] },
                },
                site,
            ));
        }
        Ok(value)
    }

    pub(crate) fn lower_terminal_call(
        &self,
        node: usize,
        body: &mut MachineBody,
        flow: &mut BlockFlow,
    ) -> Result<(CallTarget, Vec<CallArgument>, super::calls::OwedClauses, crate::PauseEffects, Vec<crate::CallRegion>), LoweringError> {
        let file = self.tree.resolved().file;
        let carrier = &self.signature.result;
        let mut call_node = node;
        while let (NodeKind::Paren | NodeKind::CallMarker, [inner]) = (file.nodes[call_node].kind, file.nodes[call_node].children.as_slice()) {
            if file.nodes[call_node].kind == NodeKind::CallMarker && !self.tree.has_acknowledged_call(*inner) {
                return Err(unsupported_node(file, call_node, "terminal call marker lacks its typed acknowledgement"));
            }
            let records = return_semantics(self.tree, call_node, carrier)?;
            if records.integer.is_some() || records.constant.is_some() {
                return Err(unsupported_node(file, call_node, "terminal call wrapper has a non-call value"));
            }
            call_node = *inner;
        }
        let call = super::calls::select_call(self, call_node, carrier, body)?;
        if self.callees.may_park(call.callee) && crate::lowering::types::uses_structural_place(carrier) {
            return Err(unsupported_node(
                file,
                call_node,
                "suspending structural results need their completion claim binding",
            ));
        }
        let mut arguments = Vec::with_capacity(call.operands.len());
        for (operand, carrier) in call.operands {
            if let Some(argument) = super::references::call_argument(self, operand, carrier, body)? {
                arguments.push(argument);
            } else {
                let result = self.lower(operand, carrier, body, flow)?;
                arguments.push(self.call_value_argument(result, body, &site_at(file, operand))?);
            }
        }
        Ok((call.callee, arguments, call.owed, call.acknowledgements, call.regions))
    }

    pub(crate) fn lower(
        &self,
        root: usize,
        carrier: &ValueType,
        block: &mut MachineBody,
        flow: &mut BlockFlow,
    ) -> Result<OperationResult, LoweringError> {
        let tree = self.tree;
        let resolved = tree.resolved();
        let file = resolved.file;
        let mut work = vec![ExpressionStep::Visit {
            node: root,
            carrier,
            landing: None,
        }];
        let mut values = Vec::new();
        while let Some(step) = work.pop() {
            let (expression, carrier, inherited) = match step {
                ExpressionStep::Frozen { node, value, carrier } => {
                    let site = site_at(file, node);
                    let ty = self
                        .types
                        .id_for(carrier)
                        .map_err(|e| LoweringError::new(&site, e.to_string()))?;
                    let nominal = match value {
                        PreparedValue::Record { owner, fields } => Some((owner, None, fields)),
                        PreparedValue::Sum { owner, case, fields } => Some((owner, Some(case), fields)),
                        _ => None,
                    };
                    if let Some((owner, case, fields)) = nominal && carrier.nominal_parts().is_some() {
                        let schema = super::construction::schema(self, ty, owner, false, &site)?;
                        if schema.cleanup_target.is_some() {
                            return Err(LoweringError::new(&site, "frozen construction cannot establish a value with cleanup"));
                        }
                        let (plan, visits) = super::construction::plan(self, carrier, schema, owner, case, fields.iter().enumerate(), &site)?;
                        work.push(ExpressionStep::Emit {
                            node,
                            output: OperationOutput::Structural(ty),
                            operation: PendingOperation::Construct(plan),
                            arity: visits.len(),
                        });
                        work.extend(
                            visits.into_iter().rev().map(|(value, carrier)| ExpressionStep::Frozen { node, value, carrier }),
                        );
                    } else if let (PreparedValue::Array(elements), ValueType::Array(element, length)) = (value, carrier) {
                        // Nominal children here are fresh complete places, not reads of live storage.
                        let supported = self.types.can_forward_value(element) || crate::lowering::types::contains_nominal_value(element);
                        work.push(array_construction(node, ty, element, *length, elements.len(), supported, &site)?);
                        work.extend(
                            elements
                                .iter()
                                .rev()
                                .map(|value| ExpressionStep::Frozen { node, value, carrier: element }),
                        );
                    } else {
                        let literal = frozen_literal(value, carrier, &site)?;
                        values.push(block.push_operation(OperationOutput::Scalar(ty), OperationKind::Constant(literal), site));
                    }
                    continue;
                }
                ExpressionStep::Qualify { node, target, change } => {
                    let Some(OperationResult::Scalar(source)) = values.pop() else {
                        return Err(unsupported_node(file, node, "scalar qualification requires a scalar value"));
                    };
                    let result = flow.change_scalar_qualification(block, source, target, change, site_at(file, node));
                    values.push(OperationResult::Scalar(result));
                    continue;
                }
                ExpressionStep::Visit { node, carrier, landing } => {
                    if file.nodes[node].kind == NodeKind::PathExpression
                        && let Some(Symbol::Binding { binder_node, .. }) = resolved.symbol(node)
                        && let Some(place) = flow.payload_bindings.get(binder_node)
                    {
                        values.push(OperationResult::Scalar(place.read_binding(self, *binder_node, node, carrier, block)?));
                        continue;
                    }
                    (node, carrier, landing)
                }
                ExpressionStep::SplitIntegerMatch { dispatch, subject_carrier, carrier, ty } => {
                    let Some(OperationResult::Scalar(value)) = values.pop() else {
                        return Err(unsupported_node(file, file.nodes[dispatch].children[0], "integer pattern needs a scalar subject"));
                    };
                    let captured = value_patterns::Subject::Integer(value, subject_carrier);
                    let targets = value_patterns::lower(self, dispatch, captured, block, flow)?;
                    work.push(ExpressionStep::MatchArms { targets, carrier, ty });
                    continue;
                }
                ExpressionStep::SplitMatch { subject, arms, carrier, ty } => {
                    let Some(OperationResult::Scalar(condition)) = values.pop() else {
                        return Err(unsupported_node(file, subject, "Boolean dispatch requires a scalar subject"));
                    };
                    let targets = crate::machines::branches::split_boolean(block, flow, file, condition, &arms);
                    work.push(ExpressionStep::MatchArms {
                        targets: targets
                            .into_iter()
                            .zip(arms)
                            .map(|(target, arm)| (target, arm.expression))
                            .collect(),
                        carrier,
                        ty,
                    });
                    continue;
                }
                ExpressionStep::MatchArms { targets, carrier, ty } => {
                    let join = flow.reserve();
                    let mut remaining = targets.into_iter();
                    let (target, first) = remaining.next().expect("the planned dispatch has an arm");
                    flow.open(block, target, None);
                    work.push(ExpressionStep::JoinMatch {
                        expression: first,
                        remaining,
                        carrier,
                        join,
                        ty,
                    });
                    work.push(ExpressionStep::Visit {
                        node: first,
                        carrier,
                        landing: None,
                    });
                    continue;
                }
                ExpressionStep::JoinMatch {
                    expression,
                    mut remaining,
                    carrier,
                    join,
                    ty,
                } => {
                    let value = values.pop().expect("each match arm produces one result");
                    let site = site_at(file, expression);
                    let argument = match (ty, value) {
                        (None, OperationResult::Unit) => None,
                        (Some(_), _) => Some(self.join_argument(value, carrier, block, &site)?),
                        _ => return Err(LoweringError::new(&site, "unit match arm produced a value")),
                    };
                    flow.jump_with_result(block, join, argument, site);
                    if let Some((target, arm)) = remaining.next() {
                        flow.open(block, target, None);
                        work.push(ExpressionStep::JoinMatch {
                            expression: arm,
                            remaining,
                            carrier,
                            join,
                            ty,
                        });
                        work.push(ExpressionStep::Visit {
                            node: arm,
                            carrier,
                            landing: None,
                        });
                    } else {
                        values.push(match ty {
                            Some(ty) if crate::lowering::types::uses_structural_place(carrier) => {
                                OperationResult::Structural(flow.open_structural_join(block, join, ty))
                            }
                            Some(ty) => OperationResult::Scalar(flow.open_scalar_join(block, join, ty)),
                            None => {
                                flow.open(block, join, None);
                                OperationResult::Unit
                            }
                        });
                    }
                    continue;
                }
                ExpressionStep::SplitBoolean {
                    node,
                    right,
                    evaluate_right_when,
                    ty,
                } => {
                    let Some(OperationResult::Scalar(left)) = values.pop() else {
                        return Err(unsupported_node(file, node, "Boolean connective requires a scalar left operand"));
                    };
                    let join = flow.begin_short_circuit(block, left, evaluate_right_when, site_at(file, right), site_at(file, node));
                    work.push(ExpressionStep::JoinBoolean { right, join, ty });
                    work.push(ExpressionStep::Visit {
                        node: right,
                        carrier: &ValueType::Bool,
                        landing: None,
                    });
                    continue;
                }
                ExpressionStep::JoinBoolean { right, join, ty } => {
                    let Some(OperationResult::Scalar(value)) = values.pop() else {
                        return Err(unsupported_node(file, right, "Boolean connective requires a scalar right operand"));
                    };
                    flow.jump_with_result(block, join, Some(CallArgument::Scalar(value)), site_at(file, right));
                    values.push(OperationResult::Scalar(flow.open_scalar_join(block, join, ty)));
                    continue;
                }
                ExpressionStep::Emit {
                    node,
                    output,
                    operation,
                    arity,
                } => {
                    let start = values
                        .len()
                        .checked_sub(arity)
                        .expect("each operand visit produces one result");
                    let mut operands = values.drain(start..);
                    let kind = match operation {
                        PendingOperation::Construct(plan) => plan.finish(self, operands, block, &site_at(file, node))?,
                        PendingOperation::Integer(_)
                        | PendingOperation::Float(_)
                        | PendingOperation::Boolean(_)
                        | PendingOperation::ExactNumericCast
                        | PendingOperation::EstablishScalarArray => {
                            let operands = operands
                                .map(|operand| match operand {
                                    OperationResult::Scalar(value) => Ok(value),
                                    _ => Err(unsupported_node(file, node, "scalar operation requires a scalar operand")),
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            match operation {
                                PendingOperation::Integer(operation) => operation.emit(operands),
                                // Every canonical float operator rounds nearest-even; directed
                                // rounding has its own named requirements, not operator syntax.
                                PendingOperation::Float(operation) => OperationKind::Float {
                                    operation,
                                    semantics: crate::FloatSemantics::Ieee754NearestEvenV1,
                                    operands,
                                },
                                PendingOperation::Boolean(primitive) => OperationKind::Boolean { primitive, operands },
                                PendingOperation::ExactNumericCast => OperationKind::ExactNumericCast { operand: operands[0] },
                                PendingOperation::EstablishScalarArray => OperationKind::EstablishScalarArray { elements: operands },
                                _ => unreachable!(),
                            }
                        }
                        PendingOperation::ScalarBitRecast {
                            access,
                            source_referent,
                            destination_referent,
                        } => {
                            let [OperationResult::Structural(source)] = *operands.as_slice() else {
                                return Err(unsupported_node(file, node, "recast requires one source descriptor"));
                            };
                            drop(operands);
                            let expected = crate::TypeShape::ScalarReference {
                                referent: source_referent,
                                access,
                            };
                            if self.types.get(block.places[source.index()].ty) != Some(&expected) {
                                return Err(unsupported_node(file, node, "recast source descriptor disagrees with its typed view"));
                            }
                            match access {
                                crate::ReferenceAccess::Shared => OperationKind::ScalarBitRecast { source, source_referent, destination_referent },
                                crate::ReferenceAccess::Mutable => OperationKind::MutableScalarBitRecast {
                                    lender: source,
                                    source_referent,
                                    destination_referent,
                                },
                                crate::ReferenceAccess::WriteOnly => return Err(unsupported_node(file, node, "write-only views admit no recast")),
                            }
                        }
                        PendingOperation::EstablishStructuralArray => {
                            // A sequence has no declared extent to count against, so
                            // each operand must be a place of exactly its element row.
                            let element = match output {
                                OperationOutput::Structural(result) => match self.types.get(result) {
                                    Some(&crate::TypeShape::ElementSequence { element }) => Some(element),
                                    _ => None,
                                },
                                _ => None,
                            };
                            let mut elements = Vec::with_capacity(arity);
                            for operand in operands {
                                let place = match (operand, element) {
                                    (OperationResult::Structural(place), _) => place,
                                    // A scalar element is a value, and the constructor takes
                                    // places: give each one its own home, in element order.
                                    (OperationResult::Scalar(value), Some(element)) if block.values[value.index()].ty == element => {
                                        let home = block.push_operation(
                                            OperationOutput::Structural(element),
                                            OperationKind::EstablishPrimitiveLocal {
                                                value,
                                                receiving_domains: None,
                                            },
                                            site_at(file, node),
                                        );
                                        let OperationResult::Structural(home) = home else {
                                            unreachable!("a primitive home is a structural place")
                                        };
                                        home
                                    }
                                    _ => return Err(unsupported_node(file, node, "structural array requires an owned place for each element")),
                                };
                                if element.is_some_and(|element| block.places[place.index()].ty != element) {
                                    return Err(unsupported_node(file, node, "sequence element disagrees with the sequence's element type"));
                                }
                                elements.push(place);
                            }
                            OperationKind::EstablishStructuralArray { elements }
                        }
                        PendingOperation::Call {
                            pause,
                            callee,
                            owed,
                            regions,
                            borrowed,
                        } => {
                            let site = site_at(file, node);
                            let arguments = borrowed
                                .into_iter()
                                .map(|argument| match argument {
                                    Some(argument) => Ok(argument),
                                    None => self.call_value_argument(operands.next().expect("each value argument was scheduled"), block, &site),
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            let operation = OperationId(next_id(block.operations.len()));
                            drop(operands);
                            super::calls::record_acknowledgement(block, crate::CallSite::Operation(operation), pause);
                            super::calls::apply_regions(block, crate::CallSite::Operation(operation), regions);
                            let requirements = super::calls::owe(block, crate::CallSite::Operation(operation), owed);
                            OperationKind::Call {
                                callee,
                                arguments,
                                requirements,
                            }
                        }
                        PendingOperation::CollectionLength(length) => {
                            let [OperationResult::Structural(source)] = *operands.as_slice() else {
                                return Err(unsupported_node(file, node, "collection length requires a structural receiver"));
                            };
                            drop(operands);
                            match length {
                                crate::machines::access::CollectionLength::Fixed(extent) => OperationKind::Constant(ScalarLiteral::Integer(extent.into())),
                                crate::machines::access::CollectionLength::Bytes => OperationKind::ByteSequenceLength { source },
                            }
                        }
                        PendingOperation::ByteSequenceRead { selector, length_type } => {
                            let [
                                OperationResult::Structural(source),
                                OperationResult::Scalar(index),
                            ] = *operands.as_slice()
                            else {
                                return Err(unsupported_node(file, node, "byte read requires a shared view and a scalar index"));
                            };
                            drop(operands);
                            // The bounds obligation uses the length observed on this very descriptor.
                            let OperationResult::Scalar(length) = block.push_operation(
                                OperationOutput::Scalar(length_type),
                                OperationKind::ByteSequenceLength { source },
                                site_at(file, node),
                            ) else {
                                unreachable!()
                            };
                            let obligation = crate::BoundsObligationId(next_id(block.diagnostics.bounds.len()));
                            block.diagnostics.bounds.push(site_at(file, selector));
                            OperationKind::ByteSequenceRead {
                                source,
                                index,
                                length,
                                obligation,
                            }
                        }
                        PendingOperation::ElementViewLength => {
                            let [OperationResult::Structural(source)] = *operands.as_slice() else {
                                return Err(unsupported_node(file, node, "element view length requires a structural descriptor"));
                            };
                            drop(operands);
                            OperationKind::ElementViewLength { source }
                        }
                        PendingOperation::ElementViewRead { selector, length_type } => {
                            let [
                                OperationResult::Structural(source),
                                OperationResult::Scalar(index),
                            ] = *operands.as_slice()
                            else {
                                return Err(unsupported_node(file, node, "element read requires a structural descriptor and a scalar index"));
                            };
                            drop(operands);
                            // The bound compares element counts observed on this very descriptor.
                            let OperationResult::Scalar(length) = block.push_operation(
                                OperationOutput::Scalar(length_type),
                                OperationKind::ElementViewLength { source },
                                site_at(file, node),
                            ) else {
                                unreachable!()
                            };
                            let obligation = crate::BoundsObligationId(next_id(block.diagnostics.bounds.len()));
                            block.diagnostics.bounds.push(site_at(file, selector));
                            OperationKind::ElementViewRead {
                                source,
                                index,
                                length,
                                obligation,
                                path: Vec::new(),
                            }
                        }
                        PendingOperation::ProjectedScalarRead(path) => {
                            let [OperationResult::Structural(root)] = *operands.as_slice() else {
                                return Err(unsupported_node(file, node, "field read requires a structural receiver"));
                            };
                            drop(operands);
                            let site = site_at(file, node);
                            let read = OperationKind::ProjectedScalarRead { source: crate::ProjectedPlace { root, path } };
                            values.push(block.push_operation(output, read, site.clone()));
                            // The projected scalar is independent once read. The expression owned
                            // its temporary receiver, including any cleanup declared for its type.
                            if super::ownership::owned(block.places[root.index()].ty, self.types, self.data) {
                                super::ownership::dispose_temporary(self, block, flow, root, site)?;
                            }
                            continue;
                        }
                        PendingOperation::FixedArrayRead(selectors) => {
                            let Some(OperationResult::Structural(source)) = operands.next() else {
                                return Err(unsupported_node(file, node, "indexed read requires an owned structural root"));
                            };
                            let mut path = Vec::with_capacity(selectors.len());
                            for (selector, operand) in selectors.into_iter().zip(operands) {
                                let OperationResult::Scalar(value) = operand else {
                                    return Err(unsupported_node(file, selector, "array selector requires a scalar value"));
                                };
                                let obligation = crate::BoundsObligationId(next_id(block.diagnostics.bounds.len()));
                                block.diagnostics.bounds.push(site_at(file, selector));
                                path.push(crate::RuntimeIndex { selector: value, obligation });
                            }
                            match output {
                                OperationOutput::Scalar(_) => OperationKind::PrimitiveScalarRead { source, path },
                                OperationOutput::Structural(_) => OperationKind::StructuralLeafCopy { source, path },
                                OperationOutput::Unit => return Err(unsupported_node(file, node, "indexed read requires a result")),
                            }
                        }
                    };
                    values.push(block.push_operation(output, kind, site_at(file, node)));
                    continue;
                }
            };
            let node = &file.nodes[expression];
            // A descriptor read keeps the actual selected value; a refinement is explicit.
            if matches!(carrier, ValueType::Callable(_)) && !matches!(node.kind, NodeKind::Call | NodeKind::Match) {
                values.push(super::runtime_calls::descriptor_value(self, expression, carrier, block)?);
                continue;
            }
            if let Some(view) = super::loans::view_at_region(self, expression, carrier, block)? {
                values.push(view);
                continue;
            }
            if crate::lowering::types::is_reference_descriptor(carrier)
                || node.kind == NodeKind::Borrow && carrier == &self.signature.result
                    && crate::declarations::regions::direct(carrier, &self.signature.machine).is_some()
            {
                if let (NodeKind::Paren, [inner]) = (node.kind, node.children.as_slice()) {
                    super::loans::check_descriptor(tree, expression, carrier, &self.signature.machine)?;
                    work.push(ExpressionStep::Visit {
                        node: *inner,
                        carrier,
                        landing: inherited,
                    });
                    continue;
                }
                if let Some(loan) = super::loans::descriptor(self, expression, carrier, block)? {
                    values.push(loan);
                    continue;
                }
            }
            let lookup_type = || {
                self.types
                    .id_for(carrier)
                    .map_err(|e| LoweringError::new(&site_at(file, expression), e.to_string()))
            };
            if let Some(construction) = tree.construction(expression) {
                let (plan, visits) = super::construction::select(self, expression, carrier, construction)?;
                work.push(ExpressionStep::Emit {
                    node: expression,
                    output: OperationOutput::Structural(lookup_type()?),
                    operation: PendingOperation::Construct(plan),
                    arity: visits.len(),
                });
                for (node, carrier) in visits.into_iter().rev() {
                    work.push(ExpressionStep::Visit { node, carrier, landing: None });
                }
                continue;
            }
            if node.kind == NodeKind::Move
                && let [operand] = node.children.as_slice()
                && super::view_loans::selects_view_element(self, *operand)
            {
                values.push(super::view_loans::move_element(self, expression, carrier, block, flow)?);
                continue;
            }
            if (crate::lowering::types::contains_nominal_value(carrier)
                || node.kind == NodeKind::FieldAccess && matches!(carrier, ValueType::Array(..)) && self.types.can_forward_value(carrier))
                && matches!(node.kind, NodeKind::Move | NodeKind::Dereference | NodeKind::FieldAccess | NodeKind::PathExpression)
                && (node.kind != NodeKind::PathExpression || matches!(resolved.symbol(expression), Some(Symbol::Binding { .. })))
            {
                values.push(super::ownership::move_value(self, expression, carrier, block)?);
                continue;
            }
            // `reflect\callable(pick)` is spelled as a call and selects a
            // declaration instead. It is a value and never reaches call selection.
            if let Some(description) = super::reflection::describe(self, expression, carrier, block)? {
                values.push(description);
                continue;
            }
            if node.kind == NodeKind::Call
                || tree
                    .semantics_at(expression)
                    .any(|s| matches!(s, NodeSemantics::Call(_)))
            {
                if let Some(citation) = super::theorem_citations::select(self.prepared, self.tree, self.signature, expression, carrier, &self.callees.theorems, |node, formal| {
                    let Symbol::Binding { binder_node, .. } = resolved.symbol(node)? else { return None };
                    let (_, OperationResult::Scalar(value)) = self.bindings.iter().rev().find(|(n, _)| n == binder_node)? else { return None };
                    (block.values[value.index()].ty == self.types.id_for(formal).ok()?).then_some(*value)
                })? {
                    let operation = OperationId(next_id(block.operations.len()));
                    let requirements = crate::declarations::contracts::call_requirements(crate::CallSite::Operation(operation), citation.requirement_count);
                    values.push(block.push_operation(OperationOutput::Unit, OperationKind::ApplyTheorem(Box::new(crate::TheoremApplication {
                        theorem: citation.theorem, arguments: citation.arguments, subjects: citation.subjects, requirements,
                    })), site_at(file, expression)));
                    continue;
                }
                let call = super::calls::select_call(self, expression, carrier, block)?;
                // A reference result at one of this machine's regions is stored
                // as the reference. The region is on the call's `CallRegions`.
                let stored = crate::declarations::regions::slot_carrier(carrier, &self.signature.machine);
                let stored_type = || {
                    self.types
                        .id_for(stored)
                        .map_err(|e| LoweringError::new(&site_at(file, expression), e.to_string()))
                };
                let output = match stored {
                    ValueType::NoResult => OperationOutput::Unit,
                    ty if crate::lowering::types::uses_structural_place(ty) => OperationOutput::Structural(stored_type()?),
                    _ => OperationOutput::Scalar(stored_type()?),
                };
                let mut borrowed = Vec::with_capacity(call.operands.len());
                let mut visits = Vec::new();
                for (operand, formal) in call.operands {
                    let argument = super::references::call_argument(self, operand, formal, block)?;
                    if argument.is_none() {
                        visits.push(ExpressionStep::Visit {
                            node: operand,
                            carrier: formal,
                            landing: None,
                        });
                    }
                    borrowed.push(argument);
                }
                work.push(ExpressionStep::Emit {
                    node: expression,
                    output,
                    operation: PendingOperation::Call {
                        borrowed,
                        callee: call.callee,
                        owed: call.owed,
                        regions: call.regions,
                        pause: call.acknowledgements,
                    },
                    arity: visits.len(),
                });
                work.extend(visits.into_iter().rev());
                continue;
            }
            // A forwarded tag belongs to the already-qualified value. Producing a
            // new member requires a separate establishment operation and obligation.
            if matches!(carrier, ValueType::Qualified(..))
                && !(matches!(node.kind, NodeKind::Paren | NodeKind::Match | NodeKind::Cast | NodeKind::Index)
                    || node.kind == NodeKind::PathExpression && matches!(resolved.symbol(expression), Some(Symbol::Binding { .. })))
            {
                return Err(unsupported_node(
                    file,
                    expression,
                    "qualified scalar requires membership establishment lowering",
                ));
            }
            // Reading through a loan or a field copies stored bits, so a float
            // reads like any other stored scalar.
            if crate::lowering::types::stored_scalar(carrier).is_ok()
                && (node.kind == NodeKind::Dereference
                    || node.kind == NodeKind::FieldAccess
                        && tree
                            .semantics_at(expression)
                            .any(|record| matches!(record, NodeSemantics::Field(_))))
            {
                let (root, path) = super::references::scalar_read(self, expression, carrier, block)?;
                let output = OperationOutput::Scalar(lookup_type()?);
                match root {
                    super::references::PlaceRoot::Bound(root) => values.push(block.push_operation(
                        output,
                        OperationKind::ProjectedScalarRead {
                            source: crate::ProjectedPlace { root, path },
                        },
                        site_at(file, expression),
                    )),
                    super::references::PlaceRoot::Expression(node, ty) => {
                        work.push(ExpressionStep::Emit {
                            node: expression,
                            output,
                            operation: PendingOperation::ProjectedScalarRead(path),
                            arity: 1,
                        });
                        work.push(ExpressionStep::Visit {
                            node,
                            carrier: ty,
                            landing: None,
                        });
                    }
                }
                continue;
            }
            if node.kind == NodeKind::FieldAccess {
                let mut selected = None;
                let mut typed = false;
                for record in tree.semantics_at(expression) {
                    match record {
                        NodeSemantics::Operation(op) if &op.result == carrier && selected.is_none() => {
                            selected = Some(
                                crate::machines::access::collection_length(op)
                                    .map(|(receiver, extent)| (receiver, PendingOperation::CollectionLength(extent)))
                                    .or_else(|| {
                                        crate::machines::access::element_view_length(op).map(|receiver| (receiver, PendingOperation::ElementViewLength))
                                    })
                                    .ok_or_else(|| unsupported_node(file, expression, "unsupported collection metadata"))?,
                            );
                        }
                        NodeSemantics::Expression(e) if &e.ty == carrier && e.static_argument_slot.is_none() => typed = true,
                        _ => return Err(unsupported_node(file, expression, "unsupported typed field access")),
                    }
                }
                let (Some((receiver, operation)), [base], true) = (selected, node.children.as_slice(), typed) else {
                    return Err(unsupported_node(file, expression, "field access has no typed collection length"));
                };
                work.push(ExpressionStep::Emit {
                    node: expression,
                    output: OperationOutput::Scalar(lookup_type()?),
                    operation,
                    arity: 1,
                });
                work.push(ExpressionStep::Visit {
                    node: *base,
                    carrier: receiver,
                    landing: None,
                });
                continue;
            }
            if node.kind == NodeKind::Index {
                let read = crate::machines::access::select_indexed_read(self.types, self.data, tree.forest(), tree, expression, carrier)?;
                let arity = 1 + read.selectors.len();
                let output = if crate::lowering::types::uses_structural_place(carrier) {
                    OperationOutput::Structural(lookup_type()?)
                } else {
                    OperationOutput::Scalar(lookup_type()?)
                };
                let mut selector_visits = Vec::with_capacity(read.selectors.len());
                for &(selector, selector_carrier) in read.selectors.iter().rev() {
                    selector_visits.push(ExpressionStep::Visit {
                        node: selector,
                        carrier: selector_carrier,
                        landing: None,
                    });
                }
                work.push(ExpressionStep::Emit {
                    node: expression,
                    output,
                    operation: if crate::lowering::types::is_shared_byte_view(read.root_carrier) {
                        let [(selector, index_type)] = read.selectors.as_slice() else {
                            return Err(unsupported_node(file, expression, "byte read requires one selector"));
                        };
                        PendingOperation::ByteSequenceRead {
                            selector: *selector,
                            length_type: self
                                .types
                                .id_for(index_type)
                                .map_err(|e| LoweringError::new(&site_at(file, *selector), e.to_string()))?,
                        }
                    } else if crate::lowering::types::scalar_element_view(read.root_carrier).is_some() {
                        let [(selector, index_type)] = read.selectors.as_slice() else {
                            return Err(unsupported_node(file, expression, "element view read requires one selector"));
                        };
                        PendingOperation::ElementViewRead {
                            selector: *selector,
                            length_type: self
                                .types
                                .id_for(index_type)
                                .map_err(|e| LoweringError::new(&site_at(file, *selector), e.to_string()))?,
                        }
                    } else {
                        PendingOperation::FixedArrayRead(read.selectors.into_iter().map(|(node, _)| node).collect())
                    },
                    arity,
                });
                work.extend(selector_visits);
                work.push(ExpressionStep::Visit {
                    node: read.root,
                    carrier: read.root_carrier,
                    landing: None,
                });
                continue;
            }
            if node.kind == NodeKind::Match {
                // A shared reference arm copies its descriptor onto the edge, like a
                // shared view; the join parameter then names the arriving loan.
                // An exclusive descriptor transfers instead (see `join_argument`).
                let shared_reference = matches!(crate::lowering::types::place_reference(carrier), Some((crate::ReferenceAccess::Shared, _)));
                if carrier != &ValueType::NoResult
                    && !self.types.can_forward_value(carrier)
                    && !crate::lowering::types::contains_nominal_value(carrier)
                    && !shared_reference
                    && !crate::lowering::types::is_exclusive_descriptor(carrier)
                {
                    return Err(unsupported_node(file, expression, "interior match requires a supported result carrier"));
                }
                let ty = (carrier != &ValueType::NoResult)
                    .then(lookup_type)
                    .transpose()?;
                crate::machines::branches::check_match_result(tree, expression, carrier, &self.signature.machine)?;
                let tested = tested_dispatch(file, expression);
                if let Some(TestedDispatch::Literal { subject }) = tested {
                    match value_patterns::plan(self, subject, block)? {
                        value_patterns::SubjectPlan::EvaluateInteger(subject_carrier) => {
                            work.push(ExpressionStep::SplitIntegerMatch { dispatch: expression, subject_carrier, carrier, ty });
                            work.push(ExpressionStep::Visit { node: subject, carrier: subject_carrier, landing: None });
                        }
                        value_patterns::SubjectPlan::Captured(subject) => {
                            let targets = value_patterns::lower(self, expression, subject, block, flow)?;
                            work.push(ExpressionStep::MatchArms { targets, carrier, ty });
                        }
                    }
                    continue;
                }
                if let Some(TestedDispatch::Guards) = tested {
                    let targets = crate::machines::branches::lower_guards(self, expression, block, flow)?;
                    work.push(ExpressionStep::MatchArms { targets, carrier, ty });
                    continue;
                }
                if let Some(targets) = super::references::dispatch_cases(self, expression, block, flow)? {
                    work.push(ExpressionStep::MatchArms { targets, carrier, ty });
                    continue;
                }
                let (subject, arms) = crate::machines::branches::boolean_dispatch(tree, expression)?;
                work.push(ExpressionStep::SplitMatch { subject, arms, carrier, ty });
                work.push(ExpressionStep::Visit {
                    node: subject,
                    carrier: &ValueType::Bool,
                    landing: None,
                });
                continue;
            }
            if inherited.is_none()
                && let Some(bits) = crate::machines::arithmetic::folded_float(tree, self.prepared, expression, carrier)?
            {
                values.push(block.push_operation(
                    OperationOutput::Scalar(lookup_type()?),
                    OperationKind::Constant(ScalarLiteral::IeeeFloat(bits)),
                    site_at(file, expression),
                ));
                continue;
            }
            if matches!(node.kind, NodeKind::Binary | NodeKind::Unary)
                && let Some(value) = crate::machines::arithmetic::folded_exact_operator(tree, expression, carrier, inherited)?
            {
                values.push(block.push_operation(
                    OperationOutput::Scalar(lookup_type()?),
                    OperationKind::Constant(ScalarLiteral::Integer(value.clone())),
                    site_at(file, expression),
                ));
                continue;
            }
            if matches!(node.kind, NodeKind::Binary | NodeKind::Unary | NodeKind::Cast) {
                let mut selected = None;
                let mut obligations = Vec::new();
                for record in tree.semantics_at(expression) {
                    match record {
                        NodeSemantics::Operation(op) if selected.is_none() && &op.result == carrier => selected = Some(op),
                        NodeSemantics::Obligation(obligation) => obligations.push(obligation),
                        NodeSemantics::Expression(e) if &e.ty == carrier && e.static_argument_slot.is_none() => {}
                        _ => return Err(unsupported_node(file, expression, "unsupported typed operator record")),
                    }
                }
                let op = selected.ok_or_else(|| unsupported_node(file, expression, "operator has no selected primitive"))?;
                if carrier == &ValueType::Bool && obligations.is_empty()
                    && let Some(value) = self.prepared.exact_comparison(tree, expression)
                {
                    values.push(block.push_operation(
                        OperationOutput::Scalar(lookup_type()?),
                        OperationKind::Constant(ScalarLiteral::Boolean(value)),
                        site_at(file, expression),
                    ));
                    continue;
                }
                if node.kind == NodeKind::Cast {
                    let operand = *node
                        .children
                        .first()
                        .ok_or_else(|| unsupported_node(file, expression, "cast lacks its operand"))?;
                    // A cast between references is a same-storage recast, never
                    // a value conversion: it keeps the source descriptor's backing.
                    if matches!(op.result, ValueType::Reference(..))
                        || op
                            .operands
                            .iter()
                            .any(|t| matches!(t, ValueType::Reference(..)))
                    {
                        let recast = super::casts::scalar_recast(op, &obligations, &self.signature.machine)?;
                        work.push(ExpressionStep::Emit {
                            node: expression,
                            output: OperationOutput::Structural(lookup_type()?),
                            operation: PendingOperation::ScalarBitRecast {
                                access: recast.access,
                                source_referent: recast.source_referent,
                                destination_referent: recast.destination_referent,
                            },
                            arity: 1,
                        });
                        work.push(ExpressionStep::Visit {
                            node: operand,
                            carrier: recast.source,
                            landing: None,
                        });
                        continue;
                    }
                    let cast = super::casts::scalar_cast(op, &obligations, self.types, &self.signature.machine)?;
                    if let Some(change) = cast.change {
                        work.push(ExpressionStep::Qualify {
                            node: expression,
                            target: lookup_type()?,
                            change,
                        });
                    }
                    if cast.convert {
                        let ty = self
                            .types
                            .id_for(carrier.carrier())
                            .map_err(|e| LoweringError::new(&op.site, e.to_string()))?;
                        work.push(ExpressionStep::Emit {
                            node: expression,
                            output: OperationOutput::Scalar(ty),
                            operation: PendingOperation::ExactNumericCast,
                            arity: 1,
                        });
                    }
                    work.push(ExpressionStep::Visit {
                        node: operand,
                        carrier: cast.operand_type,
                        landing: None,
                    });
                    continue;
                }
                if op.operands.len() != node.children.len() {
                    return Err(unsupported_node(file, expression, "operator operand count disagrees with typing"));
                }
                if matches!(op.token.as_str(), "&&" | "||") {
                    let [left, right] = *node.children.as_slice() else {
                        return Err(unsupported_node(file, expression, "Boolean connective requires two operands"));
                    };
                    if op.operands.as_slice() != [ValueType::Bool, ValueType::Bool] || *carrier != ValueType::Bool || !obligations.is_empty() {
                        return Err(unsupported_node(file, expression, "Boolean connective has incompatible typed records"));
                    }
                    work.push(ExpressionStep::SplitBoolean {
                        node: expression,
                        right,
                        evaluate_right_when: op.token == "&&",
                        ty: lookup_type()?,
                    });
                    work.push(ExpressionStep::Visit {
                        node: left,
                        carrier: &ValueType::Bool,
                        landing: None,
                    });
                    continue;
                }
                let operation = if op.result == ValueType::Bool && op.operands.iter().all(|t| *t == ValueType::Bool) && obligations.is_empty() {
                    let primitive = match (op.token.as_str(), op.operands.len()) {
                        ("!", 1) => BooleanPrimitive::Not,
                        ("==", 2) => BooleanPrimitive::Equal,
                        ("!=", 2) => BooleanPrimitive::NotEqual,
                        _ => return Err(unsupported_node(file, expression, "unsupported typed Boolean primitive")),
                    };
                    PendingOperation::Boolean(primitive)
                } else if op
                    .operands
                    .iter()
                    .all(|t| matches!(t, ValueType::Float32 | ValueType::Float64))
                {
                    PendingOperation::Float(crate::machines::arithmetic::lower_float(op, &obligations)?)
                } else {
                    PendingOperation::Integer(crate::machines::arithmetic::lower_integer(op, &obligations, &self.signature.machine)?)
                };
                work.push(ExpressionStep::Emit {
                    node: expression,
                    output: OperationOutput::Scalar(lookup_type()?),
                    operation,
                    arity: op.operands.len(),
                });
                for (&operand, carrier) in node.children.iter().zip(&op.operands).rev() {
                    work.push(ExpressionStep::Visit {
                        node: operand,
                        carrier,
                        landing: None,
                    });
                }
                continue;
            }
            let semantics = returned_semantics(tree, expression, carrier, Some(&self.signature.machine))?;
            if inherited.zip(semantics.integer).is_some_and(|(a, b)| a != b) {
                return Err(LoweringError::new(&site_at(file, expression), "conflicting exact numeric landings"));
            }
            let integer = semantics.integer.or(inherited);
            // L3 carries the decoded bytes of a quote that lands in an owned byte
            // array; establish them exactly as a frozen byte-array constant.
            if node.kind == NodeKind::Literal
                && let Some(bytes) = tree.owned_byte_literal(expression)
            {
                let site = site_at(file, expression);
                let ValueType::Array(element, length) = carrier else {
                    return Err(LoweringError::new(&site, "byte literal requires an owned byte-array carrier"));
                };
                if integer.is_some() || semantics.constant.is_some() || !matches!(element.as_ref(), ValueType::Integer { signed: false, width: Some(8) }) {
                    return Err(LoweringError::new(&site, "byte literal requires an owned byte-array carrier"));
                }
                let byte = self
                    .types
                    .id_for(element)
                    .map_err(|e| LoweringError::new(&site, e.to_string()))?;
                work.push(array_construction(
                    expression,
                    lookup_type()?,
                    element,
                    *length,
                    bytes.len(),
                    self.types.can_forward_value(element),
                    &site,
                )?);
                for &value in bytes {
                    values.push(block.push_operation(
                        OperationOutput::Scalar(byte),
                        OperationKind::Constant(ScalarLiteral::Integer(value.into())),
                        site_at(file, expression),
                    ));
                }
                continue;
            }
            let literal = match (node.kind, node.children.as_slice()) {
                (NodeKind::ArrayLiteral, elements) if integer.is_none() && semantics.constant.is_none() => {
                    // Under a by-value `[T]` the literal's own roster is the
                    // whole sequence: any count, including none. Each element
                    // lowers under the element type; the constructor homes a
                    // scalar one and takes every other as its place.
                    if let ValueType::Slice(element) = carrier {
                        work.push(ExpressionStep::Emit {
                            node: expression,
                            output: OperationOutput::Structural(lookup_type()?),
                            operation: PendingOperation::EstablishStructuralArray,
                            arity: elements.len(),
                        });
                        work.extend(elements.iter().rev().map(|&node| ExpressionStep::Visit {
                            node,
                            carrier: element,
                            landing: None,
                        }));
                        continue;
                    }
                    let ValueType::Array(element, length) = carrier else {
                        return Err(unsupported_node(file, expression, "array literal requires a fixed-array carrier"));
                    };
                    let site = site_at(file, expression);
                    // Owned nominal elements arrive as completed places, each consumed once.
                    let supported = super::fixed_arrays::owned_elements(self.types, self.data, element, *length, elements.len(), &site)?
                        || self.types.can_forward_value(element);
                    work.push(array_construction(expression, lookup_type()?, element, *length, elements.len(), supported, &site)?);
                    work.extend(elements.iter().rev().map(|&node| ExpressionStep::Visit {
                        node,
                        carrier: element,
                        landing: None,
                    }));
                    continue;
                }
                (NodeKind::Paren | NodeKind::CallMarker, [inner])
                    if semantics.constant.is_none() && (node.kind == NodeKind::Paren || tree.has_acknowledged_call(*inner)) =>
                {
                    work.push(ExpressionStep::Visit {
                        node: *inner,
                        carrier,
                        landing: integer,
                    });
                    continue;
                }
                (NodeKind::Literal, []) if semantics.constant.is_none() => {
                    let value = integer.ok_or_else(|| LoweringError::new(&site_at(file, expression), "literal has no exact integer landing"))?;
                    ScalarLiteral::Integer(value.clone())
                }
                (NodeKind::PathExpression, []) if integer.is_none() => match (semantics.constant, resolved.symbol(expression)) {
                    (Some(constant), Some(Symbol::Declaration(id))) if id == constant && id.kind == NodeKind::Const => {
                        let value = self
                            .prepared
                            .constant(constant)
                            .ok_or_else(|| unsupported_node(file, expression, "return constant has no frozen value"))?;
                        work.push(ExpressionStep::Frozen {
                            node: expression,
                            value,
                            carrier,
                        });
                        continue;
                    }
                    (None, Some(Symbol::Builtin(name))) if *carrier == ValueType::Bool && matches!(name.as_str(), "true" | "false") => {
                        ScalarLiteral::Boolean(name == "true")
                    }
                    (None, Some(Symbol::Binding { binder_node, .. })) => {
                        let value = self
                            .bindings
                            .iter()
                            .rev()
                            .find(|(binder, _)| binder == binder_node)
                            .map(|(_, v)| *v)
                            .ok_or_else(|| unsupported_node(file, expression, "binding has no lowered value"))?;
                        let actual = match value {
                            OperationResult::Scalar(value) => block.values[value.index()].ty,
                            OperationResult::Structural(place) => block.places[place.index()].ty,
                            OperationResult::Unit => return Err(unsupported_node(file, expression, "binding has no runtime value")),
                        };
                        if actual != lookup_type()? {
                            return Err(unsupported_node(file, expression, "binding has an incompatible carrier"));
                        }
                        values.push(read_local(block, value, site_at(file, expression)));
                        continue;
                    }
                    _ => return Err(unsupported_node(file, expression, "unsupported value binding")),
                },
                _ => return Err(unsupported_node(file, expression, "unsupported expression")),
            };
            values.push(block.push_operation(
                OperationOutput::Scalar(lookup_type()?),
                OperationKind::Constant(literal),
                site_at(file, expression),
            ));
        }
        Ok(values.pop().expect("the root expression produces one value"))
    }
}

#[derive(Default)]
struct ReturnSemantics<'a> {
    integer: Option<&'a BigInt>,
    constant: Option<&'a DeclarationId>,
}

pub(super) fn ungroup_place_value(
    tree: &LoweringTree<'_>,
    mut node: usize,
    carrier: &ValueType,
) -> Result<usize, LoweringError> {
    let file = tree.resolved().file;
    while let (NodeKind::Paren, [inner]) = (file.nodes[node].kind, file.nodes[node].children.as_slice()) {
        let records = return_semantics(tree, node, carrier)?;
        if records.integer.is_some() || records.constant.is_some() {
            return Err(unsupported_node(file, node, "structural wrapper has another value"));
        }
        node = *inner;
    }
    Ok(node)
}

fn return_semantics<'a>(
    tree: &'a LoweringTree<'_>,
    node: usize,
    carrier: &ValueType,
) -> Result<ReturnSemantics<'a>, LoweringError> {
    returned_semantics(tree, node, carrier, None)
}

/// `machine` is the machine whose body holds `node`, where a returned borrow
/// may be recorded on it.
fn returned_semantics<'a>(
    tree: &'a LoweringTree<'_>,
    node: usize,
    carrier: &ValueType,
    machine: Option<&DeclarationId>,
) -> Result<ReturnSemantics<'a>, LoweringError> {
    let file = tree.resolved().file;
    let mut semantic = ReturnSemantics::default();
    let mut typed = false;
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::Expression(expression)
                if expression.static_argument_slot.is_none()
                    && (&expression.ty == carrier
                        || (matches!(file.nodes[node].kind, NodeKind::Literal | NodeKind::Paren) && matches!(expression.ty, ValueType::Exact { .. }))) =>
            {
                typed = true;
            }
            NodeSemantics::Landing(landing)
                if &landing.ty == carrier
                    && landing.value.is_integer()
                    && carrier.contains_integer(landing.value.numer())
                    && semantic
                        .integer
                        .is_none_or(|value| value == landing.value.numer()) =>
            {
                semantic.integer = Some(landing.value.numer());
            }
            NodeSemantics::Constant(id) if semantic.constant.is_none_or(|previous| previous == id) => {
                semantic.constant = Some(id);
            }
            // A returned sequence that holds borrows keeps them in the operands
            // that supplied its elements, or in the formal it forwards. The
            // return edge carries that place, as for a returned record.
            NodeSemantics::Obligation(obligation)
                if crate::lowering::types::is_element_sequence(carrier)
                    && matches!(&obligation.kind, omega_typed::ObligationKind::ReturnBorrow { machine: owner } if Some(owner) == machine) => {}
            NodeSemantics::Obligation(obligation) => {
                return Err(LoweringError::new(
                    &obligation.site,
                    format!("unsupported return obligation: {:?}", obligation.kind),
                ));
            }
            _ => return Err(unsupported_node(file, node, "unsupported typed return record")),
        }
    }
    if !typed {
        return Err(LoweringError::new(&site_at(file, node), "return expression has no compatible typed carrier"));
    }
    Ok(semantic)
}

fn array_construction<'a>(
    node: usize,
    ty: TypeId,
    element: &ValueType,
    extent: u64,
    count: usize,
    supported: bool,
    site: &Site,
) -> Result<ExpressionStep<'a>, LoweringError> {
    if u64::try_from(count) != Ok(extent) || !supported {
        return Err(LoweringError::new(site, "array construction requires a matching extent and copyable elements"));
    }
    // A by-value sequence element is a descriptor in its own place, like a nested array.
    let operation = if matches!(element, ValueType::Array(..) | ValueType::Slice(_) | ValueType::Nominal(_) | ValueType::Applied(..)) {
        PendingOperation::EstablishStructuralArray
    } else {
        PendingOperation::EstablishScalarArray
    };
    Ok(ExpressionStep::Emit {
        node,
        output: OperationOutput::Structural(ty),
        operation,
        arity: count,
    })
}

/// The current value of a bound local. A primitive local homed for a borrow
/// is read from its home, since the home, not its initializer, is the local.
pub(super) fn read_local(
    body: &mut MachineBody,
    binding: OperationResult,
    site: Site,
) -> OperationResult {
    let OperationResult::Structural(home) = binding else {
        return binding;
    };
    let place = body.places[home.index()];
    match place.origin {
        crate::PlaceOrigin::Operation(id) if matches!(body.operations[id.index()].kind, OperationKind::EstablishPrimitiveLocal { .. }) => body.push_operation(
            OperationOutput::Scalar(place.ty),
            OperationKind::PrimitiveScalarRead {
                source: home,
                path: Vec::new(),
            },
            site,
        ),
        _ => binding,
    }
}

fn frozen_literal(
    value: &PreparedValue,
    carrier: &ValueType,
    site: &Site,
) -> Result<ScalarLiteral, LoweringError> {
    match value {
        PreparedValue::Bool(value) if *carrier == ValueType::Bool => Ok(ScalarLiteral::Boolean(*value)),
        PreparedValue::Integer(value) if carrier.contains_integer(value) => Ok(ScalarLiteral::Integer(value.clone())),
        // An unspecified NaN has no exact bits to store; materializing it needs
        // non-NaN proof, canonicalization or a selected exact NaN.
        PreparedValue::Float {
            value: omega_prepared::PreparedFloat::Binary32(bits),
            ..
        } if *carrier == ValueType::Float32 => Ok(ScalarLiteral::IeeeFloat(crate::IeeeFloatBits::Binary32(*bits))),
        PreparedValue::Float {
            value: omega_prepared::PreparedFloat::Binary64(bits),
            ..
        } if *carrier == ValueType::Float64 => Ok(ScalarLiteral::IeeeFloat(crate::IeeeFloatBits::Binary64(*bits))),
        PreparedValue::Float {
            value: omega_prepared::PreparedFloat::UnspecifiedNaN(_),
            ..
        } => Err(LoweringError::new(site, "an unspecified NaN constant has no exact bits to materialize")),
        _ => Err(LoweringError::new(site, "frozen constant does not match the supported return carrier")),
    }
}
