use super::destination::Destination;
use crate::evaluation::numeric_literals;
use crate::expressions::body_typer::{BodyTyper, ExpressionContext};
use crate::expressions::storage_origin::Origin;
use crate::type_error::SourceTypeError;
use crate::type_relations::TypeRelations;
use crate::type_system::substitution::TypeInference;
use crate::{ObligationKind, TypeError, ValueType};
use omega_resolved::Symbol;
use omega_syntax::NodeKind;
use omega_typed::site_at;

impl BodyTyper<'_, '_> {
    pub(super) fn constant_reference(
        &mut self,
        node: usize,
        declaration: &omega_resolved::DeclarationId,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        if self.catalog.types.indices.declaration_is_pending(declaration) {
            return Err(self.catalog.types.indices.wait(source, node));
        }
        let ty = self
            .catalog
            .constants
            .get(declaration)
            .ok_or_else(|| TypeError::unsupported(source, node))?;
        self.output
            .record_constant_reference(source.node_id(node), declaration.clone());
        Ok(ty.clone())
    }

    pub(super) fn type_expression(
        &mut self,
        node_index: usize,
    ) -> Result<ValueType, TypeError> {
        self.type_expression_with_expected_type(node_index, None)
    }
    pub(super) fn type_expression_with_expected_type(
        &mut self,
        node_index: usize,
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        self.type_at_destination(node_index, expected.into(), false)
    }

    // :ExpressionLayers. A valid run of parentheses can exhaust the command's stack
    // if each group recurses into the whole expression typer. Carry destinations
    // inward, then publish each wrapper's type and origin at its own source site.
    pub(super) fn type_at_destination(
        &mut self,
        node_index: usize,
        destination: Destination<'_>,
        machine_result: bool,
    ) -> Result<ValueType, TypeError> {
        use NodeKind::*;
        let source_file = self.unit.file;
        let mut layers = Vec::new();
        let mut node = node_index;
        let mut destination = destination;
        let mut machine_result = machine_result;
        loop {
            match source_file.nodes[node].kind {
                // A group is typed exactly as its operand.
                Paren => {}
                CallMarker => {
                    let child = self.unit.single_child(node)?;
                    if !matches!(source_file.nodes[child].kind, Call | CallMarker) {
                        return Err(TypeError::at(source_file, node, "acknowledgement requires a call"));
                    }
                    machine_result = false;
                }
                // An operator operand has no destination of its own.
                Unary => {
                    destination = Destination::Unspecified;
                    machine_result = false;
                }
                _ => break,
            }
            layers.push(node);
            node = self.unit.single_child(node)?;
        }
        let mut value = self.type_operand_node(node, destination, machine_result)?;
        for layer in layers.into_iter().rev() {
            if source_file.nodes[layer].kind == Unary {
                value = self.unary(layer, source_file.nodes[layer].children[0], value)?;
            }
            value = self.finish_expression(layer, value, None)?;
        }
        Ok(value)
    }

    /// Type one expression that is not a grouping, acknowledgement or prefix
    /// operator layer; see :ExpressionLayers.
    fn type_operand_node(
        &mut self,
        node_index: usize,
        destination: Destination<'_>,
        machine_result: bool,
    ) -> Result<ValueType, TypeError> {
        use NodeKind::*;
        let source_file = self.unit.file;
        let indices = &self.catalog.types.indices;
        let context = || destination.when_known(indices, source_file, node_index);
        let node = &source_file.nodes[node_index];
        let mut static_argument_slot = None;
        let value = match node.kind {
            PathExpression | PathType if node.kind == PathExpression || self.context == ExpressionContext::StaticArgument => {
                self.path_value(node_index, destination, &mut static_argument_slot)?
            }
            Literal
                if node.leaves[0].kind == omega_tokens::TokenKind::Quoted
                    && let Some(carrier) = self.owned_byte_literal(node_index, context()?.exact(), machine_result)? =>
            {
                carrier
            }
            Literal => {
                let (value, suffix) = numeric_literals::literal(source_file, node_index)?;
                if let Some(ty) = suffix {
                    self.check_destination(node_index, &value, &ty)?;
                    ty
                } else {
                    value
                }
            }
            Binary => return self.binary_chain(node_index),
            Cast => self.cast(node_index, context()?.exact())?,
            Tuple => ValueType::Tuple(
                node.children
                    .iter()
                    .map(|&c| self.type_expression(c))
                    .collect::<std::result::Result<_, _>>()?,
            ),
            Move => {
                let place = self.unit.single_child(node_index)?;
                let ty = self.moved_operand(place)?;
                if ty == ValueType::NoResult {
                    return Err(TypeError::at(source_file, node_index, "move requires a value"));
                }
                self.record_obligation(
                    node_index,
                    ObligationKind::Move {
                        place: site_at(source_file, place),
                        ty: ty.clone(),
                    },
                );
                ty
            }
            Call => self.type_call(node_index, context()?)?,
            Selection => crate::contracts::conformance::bind_expression(self.unit, self.catalog, node_index, self.output)?,
            RecordLiteral => self.record(node_index, context()?.exact(), machine_result)?,
            Match => self.dispatch(node_index, context()?, machine_result)?,
            Body => self.check_block(node_index, context()?, machine_result)?,
            ArrayLiteral => self.array_literal(node_index, context()?, machine_result)?,
            Borrow => {
                let place = self.unit.single_child(node_index)?;
                self.check_place(
                    place,
                    super::places::PlaceUse::Borrow {
                        site: node_index,
                        access: self.unit.access(node_index),
                    },
                )?
                .1
            }
            Dereference => {
                let (ty, access) = self.dereference_place(node_index)?;
                self.read_borrowed(node_index, &access, &ty)?;
                return Ok(ty);
            }
            FieldAccess => return self.field_chain(node_index),
            Index => self.index(node_index, None)?,
            _ => return Err(TypeError::unsupported(source_file, node_index)),
        };
        self.finish_expression(node_index, value, static_argument_slot)
    }

    /// The checks and records every typed expression ends with.
    pub(super) fn finish_expression(
        &mut self,
        node_index: usize,
        value: ValueType,
        static_argument_slot: Option<usize>,
    ) -> Result<ValueType, TypeError> {
        use NodeKind::*;
        let source_file = self.unit.file;
        let node = &source_file.nodes[node_index];
        if self.context == ExpressionContext::Execution && matches!(node.kind, Tuple | ArrayLiteral | RecordLiteral) {
            self.unit
                .check_runtime_type(node_index, &value, &self.catalog.types)?;
        }
        // Carry the value's storage origin from the operands it is made of.
        // Dispatch, blocks, records, field selection and indexing record their
        // own; every other result is a fresh value.
        let origin = match node.kind {
            PathExpression => match self.unit.symbol(node_index) {
                Some(Symbol::Binding { span, .. }) => Some(self.binding_origin(span)),
                Some(Symbol::Declaration(id)) if id.kind == Case => Some(Origin::Case(Box::new(id.clone()), Box::new(Origin::Caller))),
                _ => Some(Origin::Caller),
            },
            Paren | CallMarker | Cast | Move => Some(self.origin(node.children[0])),
            Tuple => Some(Origin::parts(node.children.iter().map(|&c| self.origin(c)).collect())),
            ArrayLiteral => Some(
                node.children
                    .iter()
                    .map(|&child| self.origin(child))
                    .reduce(|a, b| a.join(&b))
                    .unwrap_or_default(),
            ),
            Borrow => Some(self.storage_place(node.children[0]).backing),
            Match | Body | RecordLiteral | Index => None,
            _ => Some(Origin::Caller),
        };
        if let Some(origin) = origin {
            self.set_origin(node_index, origin);
        }
        value
            .validate_closed_indices()
            .map_err(|reason| TypeError::at(source_file, node_index, reason))?;
        self.catalog
            .types
            .indices
            .retain_demands_at(source_file.node_id(node_index), &value);
        self.record_expression_with_static_argument(node_index, value.clone(), static_argument_slot);
        Ok(value)
    }

    /// Apply a prefix operator to its already typed operand.
    fn unary(
        &mut self,
        node_index: usize,
        operand: usize,
        value: ValueType,
    ) -> Result<ValueType, TypeError> {
        let source_file = self.unit.file;
        let operand = self.call_operand(operand, value);
        let token = self.unit.operator_token(node_index)?;
        if let Some(ty) = self.declared_operator(node_index, token, std::slice::from_ref(&operand))? {
            return Ok(ty);
        }
        let arg = operand.value;
        let ty = match (token, &arg) {
            ("!", ValueType::Bool) | ("~", ValueType::Integer { width: Some(_), .. } | ValueType::IndexedInteger { .. }) => arg.clone(),
            ("-", ValueType::Integer { signed: true, .. } | ValueType::IndexedInteger { signed: true, .. } | ValueType::Float32 | ValueType::Float64) => {
                arg.clone()
            }
            ("-", ValueType::Exact { value, fractional_origin }) => ValueType::Exact {
                value: -value,
                fractional_origin: *fractional_origin,
            },
            _ => return Err(TypeError::at(source_file, node_index, format!("no matching unary {token} for {arg}"))),
        };
        if token == "-" && matches!(arg, ValueType::Integer { signed: true, .. } | ValueType::IndexedInteger { signed: true, .. }) {
            self.record_obligation(
                node_index,
                ObligationKind::OperationContract {
                    token: token.into(),
                    operands: vec![arg.clone()],
                },
            );
        }
        self.record_operator(node_index, token, vec![arg], &ty);
        Ok(ty)
    }

    fn array_literal(
        &mut self,
        node_index: usize,
        destination: Destination<'_>,
        machine_result: bool,
    ) -> Result<ValueType, TypeError> {
        let source_file = self.unit.file;
        let node = &source_file.nodes[node_index];
        let mut pending_elements = Vec::new();
        let element_destination = destination.array_element(node.children.len() as u64, &self.catalog.types.indices, &mut pending_elements);
        let mut entries = Vec::new();
        // Elements of an explicit array constructor are parts of the
        // same owned result, so they share its byte-literal landing.
        for &e in &node.children {
            entries.push((e, self.type_at_destination(e, element_destination, machine_result)?));
        }
        let element = element_destination.exact().cloned().or_else(|| {
            entries
                .iter()
                .find(|(_, ty)| !matches!(ty, ValueType::Exact { .. }))
                .map(|(_, ty)| ty.clone())
        });
        let Some(element) = element else {
            let indices = &self.catalog.types.indices;
            if let Some(pending) = element_destination.pending()
                && pending.permits(|element| entries.iter().all(|(_, value)| value.fits(element)))
            {
                pending.retain_demands(indices);
                return Err(indices.wait(source_file, node_index));
            }
            return Err(TypeError::at(source_file, node_index, "array literal needs an element type"));
        };
        for (e, t) in entries {
            self.check_destination(e, &t, &element)?;
        }
        // The literal builds its value at the destination, as a record literal
        // does, so at an owned `[T]` it is that `[T]`; its elements stay its
        // children. An existing `[T; N]` value still does not land there.
        if let Some(ValueType::Slice(_)) = destination.exact() {
            return Ok(ValueType::Slice(Box::new(element)));
        }
        Ok(ValueType::Array(Box::new(element), node.children.len() as u64))
    }

    fn path_value(
        &mut self,
        node_index: usize,
        destination: Destination<'_>,
        static_argument_slot: &mut Option<usize>,
    ) -> Result<ValueType, TypeError> {
        use NodeKind::*;
        let source_file = self.unit.file;
        let indices = &self.catalog.types.indices;
        let expected = || {
            destination
                .when_known(indices, source_file, node_index)
                .map(Destination::exact)
        };
        let node = &source_file.nodes[node_index];
        let symbol = self.unit.name_in_role(node_index, false);
        if matches!(symbol, Some(Symbol::DeclarationQuery { .. })) {
            return Err(TypeError::at(source_file, node_index, "a declaration query cannot be used as a value"));
        }
        if !node.children.is_empty()
            && !matches!(symbol, Some(Symbol::Declaration(id)) if matches!(id.kind, MathLet | Case))
            && !matches!(symbol,Some(Symbol::Builtin(name)) if name=="core\\Squash")
        {
            return Err(TypeError::unsupported(source_file, node_index));
        }
        Ok(match symbol {
            Some(Symbol::Binding {
                span,
                kind: ConstParam | KindedParam,
                ..
            }) => {
                let parameter = self
                    .unit
                    .parameters
                    .get(span)
                    .ok_or_else(|| TypeError::unsupported(source_file, node_index))?;
                if let ValueType::Parameter { owner, index } = parameter
                    && !self.catalog.types.admits_runtime_index(owner, *index)
                {
                    *static_argument_slot = self
                        .unit
                        .specialization_owner(node_index)
                        .and_then(|specialization| {
                            self.catalog
                                .types
                                .specialization_slot(specialization, owner, *index)
                        });
                }
                self.catalog
                    .types
                    .parameter_carrier(parameter)
                    .cloned()
                    .ok_or_else(|| TypeError::unsupported(source_file, node_index))?
            }
            Some(Symbol::Binding { span, .. }) => {
                let ty = self
                    .locals
                    .get(span)
                    .cloned()
                    .ok_or_else(|| TypeError::at(source_file, node_index, "missing type of resolved binding"))?;
                if let Some(access) = self.borrowed_binding(node_index, span) {
                    self.read_borrowed(node_index, &access, &ty)?;
                }
                ty
            }
            Some(Symbol::Builtin(name)) if name == "core\\Squash" => {
                let ValueType::Squash(source) = self.unit.annotation(node_index, &self.catalog.types)? else {
                    return Err(TypeError::unsupported(source_file, node_index));
                };
                let level = ValueType::LevelOf(source.clone());
                self.record_obligation(
                    node_index,
                    ObligationKind::FormSquash {
                        source: *source,
                        level: level.clone(),
                    },
                );
                ValueType::Universe {
                    strict: true,
                    level: Box::new(level),
                }
            }
            Some(Symbol::TargetRegister(name)) => self
                .catalog
                .assembly
                .and_then(|t| t.registers.get(name))
                .cloned()
                .ok_or_else(|| TypeError::at(source_file, node_index, format!("missing register type for {name}")))?,
            Some(Symbol::Builtin(name)) if matches!(name.as_str(), "result" | "self") => self
                .subject
                .clone()
                .ok_or_else(|| TypeError::unsupported(source_file, node_index))?,
            Some(Symbol::Declaration(id))
                if id.kind == Machine
                    && let Some(wanted @ (ValueType::Function(..) | ValueType::FamilyApplication { .. } | ValueType::Squash(_))) = expected()? =>
            {
                self.mathematical_citation(node_index, id, wanted)?;
                wanted.clone()
            }
            Some(Symbol::Declaration(id))
                if id.kind == Machine
                    && let Some(ValueType::Callable(c)) = expected()? =>
            {
                self.refine_callable(
                    node_index,
                    node_index,
                    c,
                    None,
                    &crate::MachineApplication {
                        machine: Box::new(id.clone()),
                        arguments: Vec::new(),
                        enclosing_arguments: self.catalog.types.lexical_machine_arguments(id).collect(),
                    },
                )?;
                ValueType::Callable(c.clone())
            }
            Some(Symbol::Declaration(id)) if id.kind == MathLet => {
                self.mathematical_application(node_index, node_index, Some(id), self.catalog.types.mathematical[id].clone(), &[], expected()?)?
            }
            Some(Symbol::Declaration(id)) if id.kind == Field => {
                let (owner, args) = self
                    .subject
                    .as_ref()
                    .and_then(ValueType::nominal_parts)
                    .ok_or_else(|| TypeError::unsupported(source_file, node_index))?;
                let field = id.path.rsplit([':', '\\']).next().unwrap_or(&id.path);
                self.catalog
                    .fields_at(owner, source_file, node_index)?
                    .and_then(|fs| fs.get(field))
                    .filter(|(_, field)| &field.declaration == id)
                    .map(|(_, field)| self.catalog.types.apply_type_arguments(&field.ty, owner, args))
                    .ok_or_else(|| TypeError::at(source_file, node_index, "field declaration does not belong to this subject"))?
            }
            Some(Symbol::Declaration(id)) if id.kind == Const => self.constant_reference(node_index, id)?,
            Some(Symbol::Declaration(id)) if id.kind == Case => {
                let fields = self
                    .catalog
                    .fields_at(id, source_file, node_index)?
                    .ok_or_else(|| TypeError::at(source_file, node_index, "missing field declarations"))?;
                if !fields.ordered.is_empty() {
                    return Err(TypeError::at(source_file, node_index, "case requires its payload fields"));
                }
                let carrier = self.case_type(node_index, id, expected()?)?;
                let owner = self.catalog.case_owner[id];
                if self.catalog.predicates.contains(owner) {
                    self.record_obligation(node_index, ObligationKind::EstablishData(owner.clone()));
                }
                self.record_construction(node_index, owner, Some(id), &carrier, Vec::new());
                carrier
            }
            Some(Symbol::Builtin(name)) if matches!(name.as_str(), "true" | "false") => ValueType::Bool,
            _ => return Err(TypeError::unsupported(source_file, node_index)),
        })
    }
}
