mod assembly;
mod callable;
mod calls;
mod facts;
mod indexing;
mod mathematical;
mod operators;
mod packages;
mod patterns;
mod places;
mod progress;
use super::{Access, ObligationKind, TypedOperation, TypingObligation, numeric};
use super::{Catalog, Site, TypeError, TypedCall, TypedExpression, TypedStage, Unit, ValueType};
use omega_lexer::SourceSpan;
use omega_parser::NodeKind;
use omega_resolver::Symbol;
use std::collections::{HashMap, HashSet};

struct Body<'a, 'stage> {
    unit: &'a Unit<'stage>,
    catalog: &'a Catalog<'stage>,
    locals: HashMap<SourceSpan, ValueType>,
    writable: HashSet<SourceSpan>,
    output: &'a mut TypedStage,
    subject: Option<ValueType>,
    caller: Option<omega_resolver::DeclarationId>,
    local_views: HashSet<SourceSpan>,
    version: Option<Site>,
    cases: HashMap<facts::Subject, omega_resolver::DeclarationId>,
}

impl<'a, 'stage> Body<'a, 'stage> {
    fn new(
        unit: &'a Unit<'stage>,
        catalog: &'a Catalog<'stage>,
        output: &'a mut TypedStage,
    ) -> Self {
        Self {
            unit,
            catalog,
            output,
            locals: HashMap::new(),
            writable: HashSet::new(),
            local_views: HashSet::new(),
            cases: HashMap::new(),
            version: None,
            caller: None,
            subject: None,
        }
    }
}
pub(super) fn check_static_argument(
    unit: &Unit<'_>,
    catalog: &Catalog<'_>,
    n: usize,
    expected: &ValueType,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    let mut body = Body::new(unit, catalog, output);
    let value = if unit.file.nodes[n].kind == NodeKind::PathType
        && let Some(Symbol::Declaration(id)) = unit.symbol(n)
        && id.kind == NodeKind::Const
    {
        catalog.constants[id].clone()
    } else {
        body.expected(n, Some(expected))?
    };
    body.destination(n, &value, expected)
}
pub(super) fn infer_constant(
    unit: &Unit<'_>,
    catalog: &Catalog<'_>,
    expr: usize,
    output: &mut TypedStage,
) -> Result<ValueType, TypeError> {
    let mut body = Body::new(unit, catalog, output);
    let value = body.expression(expr)?;
    if matches!(value, ValueType::Exact { .. } | ValueType::NoResult) {
        return Err(TypeError::at(&unit.file, expr, "constant requires a concrete value type"));
    }
    Ok(value)
}
pub(super) fn check_fragment(
    unit: &Unit<'_>,
    catalog: &Catalog<'_>,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    let mut body = Body::new(unit, catalog, output);
    body.block(0, None).map(|_| ())
}
pub(super) fn check_callable(
    unit: &Unit<'_>,
    catalog: &Catalog<'_>,
    n: usize,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    let f = &unit.file;
    let ValueType::Callable(c) = unit.raw_type(n, None, &catalog.types)? else {
        return Err(TypeError::unsupported(f, n));
    };
    let params = f
        .child(n, NodeKind::Params)
        .ok_or_else(|| TypeError::unsupported(f, n))?;
    let locals = f.nodes[params]
        .children
        .iter()
        .zip(&c.parameters)
        .map(|(&p, t)| {
            f.nodes[p]
                .words()
                .next()
                .map(|name| (name.span, t.clone()))
                .ok_or_else(|| TypeError::unsupported(f, p))
        })
        .collect::<Result<_, _>>()?;
    let mut body = Body {
        locals,
        subject: Some(c.result.clone()),
        ..Body::new(unit, catalog, output)
    };
    for &clause in &f.nodes[n].children {
        if f.nodes[clause].kind == NodeKind::Crashes {
            body.crash_route(clause, n)?;
        }
        if matches!(f.nodes[clause].kind, NodeKind::Requires | NodeKind::Ensures) {
            let expr = unit.single_child(clause)?;
            let ty = body.expected(expr, Some(&ValueType::Bool))?;
            body.destination(expr, &ty, &ValueType::Bool)?;
        }
    }
    Ok(())
}
pub(super) fn check_declaration(
    unit: &Unit<'_>,
    catalog: &Catalog<'_>,
    n: usize,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    let f = &unit.file;
    let id = unit.declaration(n)?;
    let sig = catalog.signatures.get(id);
    let subject = match f.nodes[n].kind {
        NodeKind::Machine | NodeKind::AsmMachine => sig.map(|s| s.result.clone()),
        NodeKind::Domain => Some(catalog.types.domains.carrier(id).clone()),
        NodeKind::Data => Some(catalog.types.open_data(id)),
        _ => None,
    };
    let mut body = Body {
        locals: sig.into_iter().flat_map(|s| s.params.iter().cloned()).collect(),
        caller: matches!(id.kind, NodeKind::Machine | NodeKind::AsmMachine).then(|| id.clone()),
        subject,
        ..Body::new(unit, catalog, output)
    };
    if f.nodes[n].kind == NodeKind::MathLet {
        let mut result = catalog.mathematical[id].clone();
        if let Some(params) = f.child(n, NodeKind::Params) {
            for &p in &f.nodes[params].children {
                let ValueType::Function(input, out) = result else {
                    return Err(TypeError::unsupported(f, p));
                };
                let name = f.nodes[p]
                    .words()
                    .next()
                    .ok_or_else(|| TypeError::unsupported(f, p))?;
                body.locals.insert(name.span, *input);
                result = *out;
            }
        }
        if f.nodes[n].leaves.iter().any(|l| l.text == "=") {
            let term = *f.nodes[n].children.last().unwrap();
            let value = body.expected(term, Some(&result))?;
            body.destination(term, &value, &result)?;
        }
        return Ok(());
    }
    if f.nodes[n].kind == NodeKind::AsmMachine {
        body.assembly(n)?;
    }
    if f.nodes[n].kind == NodeKind::Const {
        let expr = *f.nodes[n]
            .children
            .last()
            .ok_or_else(|| TypeError::unsupported(f, n))?;
        let ty = &catalog.constants[id];
        let value = body.expected(expr, Some(ty))?;
        return body.destination(expr, &value, ty);
    }
    for &part in &f.nodes[n].children {
        match f.nodes[part].kind {
            NodeKind::Requires | NodeKind::Ensures => {
                let expr = unit.single_child(part)?;
                let actual = body.expected(expr, Some(&ValueType::Bool))?;
                body.destination(expr, &actual, &ValueType::Bool)?;
            }
            NodeKind::Crashes => body.crash_route(part, n)?,
            NodeKind::Terminates => body.ranking(part, id)?,
            NodeKind::Invokes => body.invocation(part, id)?,
            NodeKind::Body => {
                let result = &sig.ok_or_else(|| TypeError::unsupported(f, n))?.result;
                body.block(part, Some(result))?;
            }
            _ => {}
        }
    }
    Ok(())
}

impl Body<'_, '_> {
    fn block(
        &mut self,
        n: usize,
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let f = &self.unit.file;
        let mut result = None;
        for &statement in &f.nodes[n].children {
            use NodeKind::*;
            match f.nodes[statement].kind {
                Let => self.binding(statement)?,
                AsmBlock => self.assembly(statement)?,
                Assign => self.assign(statement)?,
                Machine => {}
                ConstParam if f.nodes[n].kind == Fragment => {
                    let ty = self
                        .unit
                        .raw_type(self.unit.single_child(statement)?, None, &self.catalog.types)?;
                    self.output.expressions.push(TypedExpression {
                        site: Site::at(f, statement),
                        ty,
                    });
                }
                Use | Data | Const | Trait | Domain | Conformance | MathLet if f.nodes[n].kind == Fragment => {}
                Transition => result = Some(self.dispatch(statement, expected)?),
                Tail | ExpressionStatement | Discard => {
                    let expr = self.unit.single_child(statement)?;
                    let tail = f.nodes[statement].kind == Tail;
                    let value = self.expected(expr, if tail { expected } else { None })?;
                    if tail {
                        if let Some(ty) = expected {
                            self.destination(expr, &value, ty)?;
                        }
                        if f.nodes[n].parent.is_some_and(|p| f.nodes[p].kind == Machine)
                            && matches!(value, ValueType::Reference(..) | ValueType::AtLifetime(..))
                        {
                            if self.local_view(expr) {
                                return Err(TypeError::at(f, expr, "returned reference borrows invocation-local storage"));
                            }
                            self.obligation(
                                expr,
                                ObligationKind::ReturnBorrow {
                                    machine: self.unit.declaration(f.nodes[n].parent.unwrap())?.clone(),
                                },
                            );
                        }
                        result = Some(expected.cloned().unwrap_or(value));
                    } else if f.nodes[statement].kind == ExpressionStatement && value != ValueType::NoResult {
                        return Err(TypeError::at(f, expr, "value-producing statement requires an explicit discard"));
                    }
                }
                _ => return Err(TypeError::unsupported(f, statement)),
            }
        }
        if result.is_none()
            && let Some(ty) = expected
            && *ty != ValueType::NoResult
        {
            return Err(TypeError::at(f, n, format!("missing result of type {ty}")));
        }
        Ok(result.unwrap_or(ValueType::NoResult))
    }
    fn binding(
        &mut self,
        n: usize,
    ) -> Result<(), TypeError> {
        let f = &self.unit.file;
        if !f.nodes[n].leaves.iter().any(|l| l.text == "=") {
            let [pattern, annotation] = *f.nodes[n].children.as_slice() else {
                return Err(TypeError::unsupported(f, n));
            };
            let ty = self.unit.annotation(annotation, &self.catalog.types)?;
            self.obligation(n, ObligationKind::ZeroValue(ty.clone()));
            if f.nodes[n].words().any(|w| w.text == "mut") {
                self.writable.insert(f.span(pattern));
            }
            if !self.bind_pattern(pattern, &ty)?.is_total() {
                return Err(TypeError::at(f, pattern, "let requires an irrefutable pattern"));
            }
            return Ok(());
        }
        let (pattern, annotation, expr) = match *f.nodes[n].children.as_slice() {
            [p, e] => (p, None, e),
            [p, t, e] => (p, Some(t), e),
            _ => return Err(TypeError::unsupported(f, n)),
        };
        let declared = annotation
            .map(|t| self.unit.annotation(t, &self.catalog.types))
            .transpose()?;
        let value = self.expected(expr, declared.as_ref())?;
        let local_type = if let Some(expected) = declared {
            self.destination(expr, &value, &expected)?;
            expected
        } else {
            if matches!(value, ValueType::Exact { .. }) {
                return Err(TypeError::at(
                    f,
                    expr,
                    "an unsuffixed numeric binding must declare its type, as in `let count: u8 = 5`",
                ));
            }
            value
        };
        if local_type == ValueType::NoResult {
            return Err(TypeError::at(f, expr, "binding requires a value"));
        }
        if f.nodes[n].words().any(|w| w.text == "mut") {
            self.writable.insert(f.span(pattern));
        }
        if self.local_view(expr) {
            self.local_views.insert(f.span(pattern));
        }
        if !self.bind_pattern(pattern, &local_type)?.is_total() {
            return Err(TypeError::at(f, pattern, "let requires an irrefutable pattern"));
        }
        Ok(())
    }
    fn destination(
        &mut self,
        n: usize,
        value: &ValueType,
        expected: &ValueType,
    ) -> Result<(), TypeError> {
        self.land(n, value, expected, false)
    }
    fn local_view(
        &self,
        n: usize,
    ) -> bool {
        let f = &self.unit.file;
        match f.nodes[n].kind {
            NodeKind::Paren => self.unit.single_child(n).is_ok_and(|i| self.local_view(i)),
            NodeKind::PathExpression => matches!(self.unit.symbol(n),Some(Symbol::Binding{span,..}) if self.local_views.contains(span)),
            NodeKind::Match | NodeKind::Transition => f.nodes[n]
                .children
                .iter()
                .filter(|&&i| f.nodes[i].kind == NodeKind::Arm)
                .any(|&arm| {
                    f.nodes[arm]
                        .children
                        .last()
                        .is_some_and(|&result| self.local_view(result))
                }),
            NodeKind::Body | NodeKind::Tail => f.nodes[n]
                .children
                .last()
                .is_some_and(|&last| self.local_view(last)),
            NodeKind::Borrow => {
                let Some(&place) = f.nodes[n].children.first() else {
                    return false;
                };
                // Borrowing the carrier itself borrows invocation-local storage,
                // even when a by-value parameter carries a reference.
                let mut root = place;
                while matches!(f.nodes[root].kind, NodeKind::FieldAccess | NodeKind::Index | NodeKind::Paren) {
                    let Some(&inner) = f.nodes[root].children.first() else {
                        return true;
                    };
                    root = inner;
                    if matches!(f.nodes[root].kind, NodeKind::PathExpression)
                        && let Some(Symbol::Binding { span, .. }) = self.unit.symbol(root)
                        && matches!(self.locals.get(span), Some(ValueType::Reference(..) | ValueType::AtLifetime(..)))
                    {
                        return self.local_views.contains(span);
                    }
                }
                matches!(self.unit.symbol(root), Some(Symbol::Binding { .. })) || f.nodes[root].kind == NodeKind::RecordLiteral
            }
            _ => false,
        }
    }
    fn land(
        &mut self,
        n: usize,
        value: &ValueType,
        expected: &ValueType,
        explicit: bool,
    ) -> Result<(), TypeError> {
        if let (ValueType::Callable(actual), ValueType::Callable(destination)) = (value, expected)
            && actual.contract != destination.contract
            && destination.admits_contract(actual)
        {
            self.obligation(
                n,
                ObligationKind::RefineCallableValue {
                    source: actual.contract.clone(),
                    destination: destination.contract.clone(),
                },
            );
        }
        if let ValueType::AtLifetime(inner, _) = expected
            && value == inner.as_ref()
        {
            self.obligation(
                n,
                ObligationKind::BindLifetime {
                    reference: value.clone(),
                    destination: expected.clone(),
                },
            );
            return Ok(());
        }
        if let ValueType::Reference(Access::Shared | Access::Mut, inner) = value
            && inner.as_ref() == expected
            && (expected.numeric()
                || expected == &ValueType::Bool
                || expected.nominal_parts().is_some_and(|(id, _)| {
                    self.catalog
                        .data_properties
                        .get(id)
                        .is_some_and(|p| p.multiplicity == super::Multiplicity::Copy)
                }))
        {
            self.obligation(n, ObligationKind::CopyRead(expected.clone()));
            return Ok(());
        }
        if !value.fits(expected) {
            return Err(TypeError::at(&self.unit.file, n, format!("expected {expected}, found {value}")));
        }
        if let ValueType::Exact {
            fractional_origin: Some(origin),
            ..
        } = value
            && matches!(expected, ValueType::Integer { .. } | ValueType::IndexedInteger { .. })
            && !explicit
        {
            return Err(TypeError::at(
                &self.unit.file,
                n,
                format!(
                    "fractional intermediate at bytes {}..{} yields {value}; use an explicit cast to {expected}",
                    origin.get_start_offset(),
                    origin.get_end_offset()
                ),
            ));
        }
        if matches!(value, ValueType::Exact { .. }) && matches!(expected, ValueType::IndexedInteger { .. }) {
            self.obligation(
                n,
                ObligationKind::ExactCast {
                    source: value.clone(),
                    destination: expected.clone(),
                },
            );
        }
        if matches!(value, ValueType::Exact { .. }) {
            self.output.landings.push(TypedExpression {
                site: Site::at(&self.unit.file, n),
                ty: expected.clone(),
            });
        }
        Ok(())
    }
    fn expression(
        &mut self,
        n: usize,
    ) -> Result<ValueType, TypeError> {
        self.expected(n, None)
    }
    fn expected(
        &mut self,
        n: usize,
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        use NodeKind::*;
        let f = &self.unit.file;
        let node = &f.nodes[n];
        let value = match node.kind {
            PathExpression => {
                let symbol = self.unit.symbol(n);
                if !node.children.is_empty() && !matches!(symbol, Some(Symbol::Declaration(id)) if id.kind == MathLet) {
                    return Err(TypeError::unsupported(f, n));
                }
                match symbol {
                    Some(Symbol::Binding {
                        span,
                        kind: ConstParam | KindedParam,
                    }) => self
                        .unit
                        .parameters
                        .get(span)
                        .and_then(|p| self.catalog.types.parameter_carrier(p))
                        .cloned()
                        .ok_or_else(|| TypeError::unsupported(f, n))?,
                    Some(Symbol::Binding { span, .. }) => self
                        .locals
                        .get(span)
                        .cloned()
                        .ok_or_else(|| TypeError::at(f, n, "missing type of resolved binding"))?,
                    Some(Symbol::TargetRegister(name)) => self
                        .catalog
                        .assembly
                        .and_then(|t| t.registers.get(name))
                        .cloned()
                        .ok_or_else(|| TypeError::at(f, n, format!("missing register type for {name}")))?,
                    Some(Symbol::Builtin(name)) if matches!(name.as_str(), "result" | "self") => self
                        .subject
                        .clone()
                        .ok_or_else(|| TypeError::unsupported(f, n))?,
                    Some(Symbol::Declaration(id)) if id.kind == Machine && matches!(expected, Some(ValueType::Callable(_))) => {
                        let Some(ValueType::Callable(c)) = expected else { unreachable!() };
                        self.refine_callable(n, c, id)?;
                        ValueType::Callable(c.clone())
                    }
                    Some(Symbol::Declaration(id)) if id.kind == MathLet => {
                        self.mathematical_application(n, n, Some(id), self.catalog.mathematical[id].clone(), &[], expected)?
                    }
                    Some(Symbol::Declaration(id)) if id.kind == Field => {
                        let (owner, args) = self
                            .subject
                            .as_ref()
                            .and_then(ValueType::nominal_parts)
                            .ok_or_else(|| TypeError::unsupported(f, n))?;
                        let field = id.path.rsplit("::").next().unwrap_or(&id.path);
                        self.catalog
                            .fields
                            .get(owner)
                            .and_then(|fs| fs.get(field))
                            .map(|t| self.catalog.types.field_type(t, owner, args))
                            .ok_or_else(|| TypeError::unsupported(f, n))?
                    }
                    Some(Symbol::Declaration(id)) if id.kind == Const => self
                        .catalog
                        .constants
                        .get(id)
                        .cloned()
                        .ok_or_else(|| TypeError::unsupported(f, n))?,
                    Some(Symbol::Declaration(id)) if id.kind == Case => {
                        if !self.catalog.fields[id].is_empty() {
                            return Err(TypeError::at(f, n, "case requires its payload fields"));
                        }
                        self.case_type(n, id, expected)?
                    }
                    Some(Symbol::Builtin(name)) if matches!(name.as_str(), "true" | "false") => ValueType::Bool,
                    _ => return Err(TypeError::unsupported(f, n)),
                }
            }
            Literal => {
                let (value, suffix) = numeric::literal(f, n)?;
                if let Some(ty) = suffix {
                    self.destination(n, &value, &ty)?;
                    ty
                } else {
                    value
                }
            }
            Binary => return self.binary_chain(n),
            Unary => {
                let arg = self.expression(self.unit.single_child(n)?)?;
                let token = node.leaves[0].text;
                if let Some(ty) = self.declared_operator(n, token, &[(self.unit.single_child(n)?, &arg)])? {
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
                        _ => return Err(TypeError::at(f, n, format!("no matching unary {token} for {arg}"))),
                    };
                    self.record_operator(n, token, vec![arg], &ty);
                    ty
                }
            }
            Cast => self.cast(n, expected)?,
            Tuple => ValueType::Tuple(
                node.children
                    .iter()
                    .map(|&c| self.expression(c))
                    .collect::<std::result::Result<_, _>>()?,
            ),
            Move => {
                let place = self.unit.single_child(n)?;
                let ty = self.expression(place)?;
                if ty == ValueType::NoResult {
                    return Err(TypeError::at(f, n, "move requires a value"));
                }
                self.obligation(
                    n,
                    ObligationKind::Move {
                        place: Site::at(f, place),
                        ty: ty.clone(),
                    },
                );
                ty
            }
            Paren => self.expected(self.unit.single_child(n)?, expected)?,
            CallMarker => {
                let child = self.unit.single_child(n)?;
                if !matches!(f.nodes[child].kind, Call | CallMarker) {
                    return Err(TypeError::at(f, n, "acknowledgement requires a call"));
                }
                self.expected(child, expected)?
            }
            Call => self.call(n, expected)?,
            RecordLiteral => self.record(n, expected)?,
            Match => self.dispatch(n, expected)?,
            Body => self.block(n, expected)?,
            ArrayLiteral => {
                let element = match expected {
                    Some(ValueType::Array(t, size)) if *size == node.children.len() as u64 => Some(t.as_ref().clone()),
                    _ => None,
                };
                let mut entries = Vec::new();
                for &e in &node.children {
                    entries.push((e, self.expected(e, element.as_ref())?));
                }
                let element = element
                    .or_else(|| {
                        entries
                            .iter()
                            .find(|(_, t)| !matches!(t, ValueType::Exact { .. }))
                            .map(|(_, t)| t.clone())
                    })
                    .ok_or_else(|| TypeError::at(f, n, "array literal needs an element type"))?;
                for (e, t) in entries {
                    self.destination(e, &t, &element)?;
                }
                ValueType::Array(Box::new(element), node.children.len() as u64)
            }
            Borrow if f.nodes[self.unit.single_child(n)?].kind == Index => self.index(self.unit.single_child(n)?, Some(self.unit.access(n)))?,
            Borrow => {
                let arg = self.unit.single_child(n)?;
                let access = self.unit.access(n);
                let ty = if access != super::Access::Shared && f.nodes[arg].kind == FieldAccess {
                    self.project_fields(arg, true)?
                } else {
                    self.expression(arg)?
                };
                if access != super::Access::Shared {
                    self.require_writable(arg)?;
                }
                self.obligation(
                    n,
                    ObligationKind::Borrow {
                        access: access.clone(),
                        place: Site::at(f, arg),
                    },
                );
                ValueType::Reference(access, Box::new(ty))
            }
            FieldAccess => return self.field_chain(n),
            Index => self.index(n, None)?,
            _ => return Err(TypeError::unsupported(f, n)),
        };
        value
            .validate_closed_indices()
            .map_err(|reason| TypeError::at(f, n, reason))?;
        self.output.expressions.push(TypedExpression {
            site: Site::at(f, n),
            ty: value.clone(),
        });
        Ok(value)
    }
    fn obligation(
        &mut self,
        n: usize,
        kind: ObligationKind,
    ) {
        self.output.obligations.push(TypingObligation {
            site: Site::at(&self.unit.file, n),
            kind,
        });
    }
}
