use crate::evaluation::numeric_literals;
use crate::expressions::body_typer::BodyTyper;
use crate::expressions::storage_origin::Origin;
use crate::{ObligationKind, Site, TypeError, TypedExpression, ValueType};
use omega_parser::NodeKind;
use omega_resolver::Symbol;

impl BodyTyper<'_, '_> {
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
        use NodeKind::*;
        let source_file = self.unit.file;
        let node = &source_file.nodes[node_index];
        let value = match node.kind {
            PathExpression => {
                let symbol = self.unit.symbol(node_index);
                if !node.children.is_empty()
                    && !matches!(symbol, Some(Symbol::Declaration(id)) if id.kind == MathLet)
                    && !matches!(symbol,Some(Symbol::Builtin(name)) if name=="core::Squash")
                {
                    return Err(TypeError::unsupported(source_file, node_index));
                }
                match symbol {
                    Some(Symbol::Binding {
                        span,
                        kind: ConstParam | KindedParam,
                        ..
                    }) => self
                        .unit
                        .parameters
                        .get(span)
                        .and_then(|p| self.catalog.types.parameter_carrier(p))
                        .cloned()
                        .ok_or_else(|| TypeError::unsupported(source_file, node_index))?,
                    Some(Symbol::Binding { span, .. }) => self
                        .locals
                        .get(span)
                        .cloned()
                        .ok_or_else(|| TypeError::at(source_file, node_index, "missing type of resolved binding"))?,
                    Some(Symbol::Builtin(name)) if name == "core::Squash" => {
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
                            && let Some(wanted @ (ValueType::Function(..) | ValueType::FamilyApplication { .. } | ValueType::Squash(_))) = expected =>
                    {
                        self.mathematical_citation(node_index, id, wanted)?;
                        wanted.clone()
                    }
                    Some(Symbol::Declaration(id)) if id.kind == Machine && matches!(expected, Some(ValueType::Callable(_))) => {
                        let Some(ValueType::Callable(c)) = expected else { unreachable!() };
                        self.refine_callable(node_index, c, id)?;
                        ValueType::Callable(c.clone())
                    }
                    Some(Symbol::Declaration(id)) if id.kind == MathLet => {
                        self.mathematical_application(node_index, node_index, Some(id), self.catalog.types.mathematical[id].clone(), &[], expected)?
                    }
                    Some(Symbol::Declaration(id)) if id.kind == Field => {
                        let (owner, args) = self
                            .subject
                            .as_ref()
                            .and_then(ValueType::nominal_parts)
                            .ok_or_else(|| TypeError::unsupported(source_file, node_index))?;
                        let field = id.path.rsplit("::").next().unwrap_or(&id.path);
                        self.catalog
                            .fields
                            .get(owner)
                            .and_then(|fs| fs.get(field))
                            .map(|t| self.catalog.types.field_type(t, owner, args))
                            .ok_or_else(|| TypeError::unsupported(source_file, node_index))?
                    }
                    Some(Symbol::Declaration(id)) if id.kind == Const => self
                        .catalog
                        .constants
                        .get(id)
                        .cloned()
                        .ok_or_else(|| TypeError::unsupported(source_file, node_index))?,
                    Some(Symbol::Declaration(id)) if id.kind == Case => {
                        if !self.catalog.fields[id].is_empty() {
                            return Err(TypeError::at(source_file, node_index, "case requires its payload fields"));
                        }
                        self.case_type(node_index, id, expected)?
                    }
                    Some(Symbol::Builtin(name)) if matches!(name.as_str(), "true" | "false") => ValueType::Bool,
                    _ => return Err(TypeError::unsupported(source_file, node_index)),
                }
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
            Unary => {
                let arg = self.type_expression(self.unit.single_child(node_index)?)?;
                let token = self.unit.operator_token(node_index)?;
                if let Some(ty) = self.declared_operator(node_index, token, &[(self.unit.single_child(node_index)?, &arg)])? {
                    ty
                } else {
                    let ty = match (token, &arg) {
                        ("!", ValueType::Bool) | ("~", ValueType::Integer { width: Some(_), .. } | ValueType::IndexedInteger { .. }) => arg.clone(),
                        (
                            "-",
                            ValueType::Integer { signed: true, .. } | ValueType::IndexedInteger { signed: true, .. } | ValueType::Float32 | ValueType::Float64,
                        ) => arg.clone(),
                        ("-", ValueType::Exact { value, fractional_origin }) => ValueType::Exact {
                            value: -value,
                            fractional_origin: *fractional_origin,
                        },
                        _ => return Err(TypeError::at(source_file, node_index, format!("no matching unary {token} for {arg}"))),
                    };
                    self.record_operator(node_index, token, vec![arg], &ty);
                    ty
                }
            }
            Cast => self.cast(node_index, expected)?,
            Tuple => ValueType::Tuple(
                node.children
                    .iter()
                    .map(|&c| self.type_expression(c))
                    .collect::<std::result::Result<_, _>>()?,
            ),
            Move => {
                let place = self.unit.single_child(node_index)?;
                let ty = self.type_expression(place)?;
                if ty == ValueType::NoResult {
                    return Err(TypeError::at(source_file, node_index, "move requires a value"));
                }
                self.record_obligation(
                    node_index,
                    ObligationKind::Move {
                        place: Site::at(source_file, place),
                        ty: ty.clone(),
                    },
                );
                ty
            }
            Paren => self.type_expression_with_expected_type(self.unit.single_child(node_index)?, expected)?,
            CallMarker => {
                let child = self.unit.single_child(node_index)?;
                if !matches!(source_file.nodes[child].kind, Call | CallMarker) {
                    return Err(TypeError::at(source_file, node_index, "acknowledgement requires a call"));
                }
                self.type_expression_with_expected_type(child, expected)?
            }
            Call => self.type_call(node_index, expected)?,
            RecordLiteral => self.record(node_index, expected)?,
            Match => self.dispatch(node_index, expected)?,
            Body => self.check_block(node_index, expected)?,
            ArrayLiteral => {
                let element = match expected {
                    Some(ValueType::Array(t, size)) if *size == node.children.len() as u64 => Some(t.as_ref().clone()),
                    _ => None,
                };
                let mut entries = Vec::new();
                for &e in &node.children {
                    entries.push((e, self.type_expression_with_expected_type(e, element.as_ref())?));
                }
                let element = element
                    .or_else(|| {
                        entries
                            .iter()
                            .find(|(_, t)| !matches!(t, ValueType::Exact { .. }))
                            .map(|(_, t)| t.clone())
                    })
                    .ok_or_else(|| TypeError::at(source_file, node_index, "array literal needs an element type"))?;
                for (e, t) in entries {
                    self.check_destination(e, &t, &element)?;
                }
                ValueType::Array(Box::new(element), node.children.len() as u64)
            }
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
            FieldAccess => return self.field_chain(node_index),
            Index => self.index(node_index, None)?,
            _ => return Err(TypeError::unsupported(source_file, node_index)),
        };
        // Carry the value's storage origin from the operands it is made of.
        // Dispatch, blocks, records, field selection and indexing record their
        // own; every other result is a fresh value.
        let origin = match node.kind {
            PathExpression => match self.unit.symbol(node_index) {
                Some(Symbol::Binding { span, .. }) => Some(self.binding_origin(span)),
                _ => Some(Origin::Caller),
            },
            Paren | CallMarker | Cast | Move => Some(self.origin(node.children[0])),
            Tuple => Some(Origin::parts(node.children.iter().map(|&c| self.origin(c)).collect())),
            ArrayLiteral => Some(
                node.children
                    .iter()
                    .fold(Origin::Caller, |all, &c| all.join(&self.origin(c).whole())),
            ),
            Borrow => Some(self.backing(node.children[0])),
            Match | Body | RecordLiteral | Index => None,
            _ => Some(Origin::Caller),
        };
        if let Some(origin) = origin {
            self.set_origin(node_index, origin);
        }
        value
            .validate_closed_indices()
            .map_err(|reason| TypeError::at(source_file, node_index, reason))?;
        self.output.expressions.push(TypedExpression {
            node: source_file.node_id(node_index),
            site: Site::at(source_file, node_index),
            ty: value.clone(),
        });
        Ok(value)
    }
}
