use crate::expressions::body_typer::BodyTyper;
use crate::expressions::storage_origin::{Origin, PatternSource};
use crate::{Access, TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};
use std::collections::{HashMap, HashSet};

pub(super) enum PatternCoverage {
    All,
    Bool(bool),
    Case(Box<DeclarationId>),
    Partial,
    Product(Vec<PatternCoverage>),
}
impl PatternCoverage {
    pub(super) fn is_total(&self) -> bool {
        matches!(self, Self::All)
    }
}

impl BodyTyper<'_, '_> {
    pub(super) fn dispatch(
        &mut self,
        node_index: usize,
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        let mut parts = source.nodes[node_index].children.iter().copied().peekable();
        let subject_place = parts.peek().and_then(|&i| self.fact_subject(i));
        let mut subject_source = PatternSource::default();
        let subject = if let Some(expr) = parts.next_if(|&i| source.nodes[i].kind != NodeKind::Arm) {
            let ty = self.type_expression(expr)?;
            subject_source = PatternSource::of(self.origin(expr));
            Some((expr, ty))
        } else {
            None
        };
        let mut all = false;
        let mut booleans = [false; 2];
        let mut cases = HashSet::new();
        let mut values = Vec::new();
        let mut products = Vec::new();
        let entry_cases = self.cases.clone();
        // An arm can reassign a binding declared before the dispatch. After
        // the dispatch the binding may hold what any arm left in it.
        let entry_origins = self.binding_origins.clone();
        let entry_version = self.version.clone();
        let mut changed = false;
        let mut merged_origins: Option<HashMap<_, Origin>> = None;
        let mut result_origin = Origin::Caller;
        for arm in parts {
            self.cases.clone_from(&entry_cases);
            self.binding_origins.clone_from(&entry_origins);
            self.version.clone_from(&entry_version);
            let [pattern, expr] = *source.nodes[arm].children.as_slice() else {
                return Err(TypeError::unsupported(source, arm));
            };
            let cover = if let Some((initializer, ty)) = &subject {
                self.bind_pattern(pattern, ty, *initializer, &subject_source)?
            } else {
                self.guard(pattern)?
            };
            match cover {
                PatternCoverage::All => all = true,
                PatternCoverage::Bool(b) => booleans[usize::from(b)] = true,
                PatternCoverage::Case(id) => {
                    if let Some(subject) = subject_place {
                        self.cases.insert(subject, *id.clone());
                    }
                    cases.insert(*id);
                }
                PatternCoverage::Product(parts) => products.push(parts),
                PatternCoverage::Partial => {}
            }
            let value = self.type_expression_with_expected_type(expr, expected)?;
            // A terminal transition returns the selected arm directly. Attach
            // its return obligation to that expression while its facts are in scope.
            if source.nodes[node_index].kind == NodeKind::Transition
                && let Some(body) = source.nodes[node_index].parent
                && let Some(result) = expected
            {
                self.check_return_borrow(body, expr, result)?;
            }
            result_origin = result_origin.join(&self.origin(expr));
            values.push((expr, value));
            changed |= self.version != entry_version;
            let merged = merged_origins.get_or_insert_default();
            for (span, origin) in &self.binding_origins {
                let merged = merged.entry(*span).or_insert(Origin::Caller);
                *merged = origin.join(merged);
            }
        }
        self.binding_origins = merged_origins.unwrap_or(entry_origins);
        self.set_origin(node_index, result_origin);
        self.version = if changed { Some(crate::Site::at(source, node_index)) } else { entry_version };
        self.cases = if changed { Default::default() } else { entry_cases };
        let subject = subject.as_ref().map(|(_, ty)| pattern_subject(ty));
        let complete_sum = subject
            .and_then(ValueType::nominal_parts)
            .and_then(|(id, _)| self.catalog.cases.get(id))
            .is_some_and(|ids| ids.iter().all(|id| cases.contains(*id)));
        if !(all || product_covers(&products) || subject == Some(&ValueType::Bool) && booleans == [true, true] || complete_sum) {
            return Err(TypeError::at(source, node_index, "dispatch does not cover its subject"));
        }
        let target = expected
            .cloned()
            .or_else(|| {
                values
                    .iter()
                    .find(|(_, t)| !matches!(t, ValueType::Exact { .. }))
                    .map(|(_, t)| t.clone())
            })
            .ok_or_else(|| TypeError::at(source, node_index, "dispatch needs a result type"))?;
        for (expr, value) in values {
            self.check_destination(expr, &value, &target)?;
        }
        Ok(target)
    }
    fn guard(
        &mut self,
        node_index: usize,
    ) -> Result<PatternCoverage, TypeError> {
        let source = self.unit.file;
        match source.nodes[node_index].kind {
            NodeKind::WildcardPattern => Ok(PatternCoverage::All),
            NodeKind::ParenPattern => self.guard(self.unit.single_child(node_index)?),
            NodeKind::ValuePattern => {
                let expr = self.unit.single_child(node_index)?;
                let value = self.type_expression(expr)?;
                self.check_destination(expr, &value, &ValueType::Bool)?;
                Ok(PatternCoverage::Partial)
            }
            NodeKind::BindingPattern => {
                let ty = self.pattern_value(node_index)?;
                self.check_destination(node_index, &ty, &ValueType::Bool)?;
                Ok(if matches!(self.unit.symbol(node_index),Some(Symbol::Builtin(s)) if s=="true") {
                    PatternCoverage::All
                } else {
                    PatternCoverage::Partial
                })
            }
            _ => Err(TypeError::unsupported(source, node_index)),
        }
    }
    fn pattern_value(
        &self,
        node_index: usize,
    ) -> Result<ValueType, TypeError> {
        match self.unit.symbol(node_index) {
            Some(Symbol::Builtin(s)) if matches!(s.as_str(), "true" | "false") => Ok(ValueType::Bool),
            Some(Symbol::Binding { span, .. }) => self
                .locals
                .get(span)
                .cloned()
                .ok_or_else(|| TypeError::unsupported(self.unit.file, node_index)),
            Some(Symbol::Declaration(id)) if id.kind == NodeKind::Const => Ok(self.catalog.constants[id].clone()),
            _ => Err(TypeError::unsupported(self.unit.file, node_index)),
        }
    }
    /// A value pattern compares against the referent of a readable reference
    /// subject, as a case pattern does. Reading a copyable primitive through it
    /// is an ordinary copy read (domains.md), recorded at `read`, the
    /// expression whose referent is read; a non-copy referent refuses there.
    fn compare_value(
        &mut self,
        pattern: usize,
        value: &ValueType,
        subject: &ValueType,
        read: usize,
    ) -> Result<(), TypeError> {
        let reference = match subject {
            ValueType::AtLifetime(reference, _) => reference.as_ref(),
            other => other,
        };
        let compared = match reference {
            ValueType::Reference(Access::Write, _) => {
                return Err(TypeError::at(self.unit.file, pattern, "cannot read through a write-only reference"));
            }
            ValueType::Reference(_, inner) => {
                self.land_value(read, subject, inner, false)?;
                inner.as_ref()
            }
            _ => subject,
        };
        self.check_destination(pattern, value, compared)
    }
    /// A tuple literal initializer supplies each component from its own
    /// expression, so a refusal or copy read names that component.
    fn component(
        &self,
        initializer: usize,
        index: usize,
        arity: usize,
    ) -> usize {
        let source = self.unit.file;
        let mut node = initializer;
        while source.nodes[node].kind == NodeKind::Paren
            && let [inner] = *source.nodes[node].children.as_slice()
        {
            node = inner;
        }
        match source.nodes[node].children.as_slice() {
            parts if source.nodes[node].kind == NodeKind::Tuple && parts.len() == arity => parts[index],
            _ => initializer,
        }
    }
    /// `initializer` is the expression whose value the pattern receives: a let
    /// initializer, a dispatch subject or one of their components.
    pub(super) fn bind_pattern(
        &mut self,
        node_index: usize,
        subject: &ValueType,
        initializer: usize,
        from: &PatternSource,
    ) -> Result<PatternCoverage, TypeError> {
        use NodeKind::*;
        let source = self.unit.file;
        let node = &source.nodes[node_index];
        match node.kind {
            WildcardPattern => Ok(PatternCoverage::All),
            BindingPattern if self.unit.symbol(node_index).is_none() => {
                self.require_declared_number(initializer, subject)?;
                self.locals.insert(source.span(node_index), subject.clone());
                self.bind_source(source.span(node_index), from);
                Ok(PatternCoverage::All)
            }
            BindingPattern => {
                let ty = self.pattern_value(node_index)?;
                self.compare_value(node_index, &ty, subject, initializer)?;
                Ok(match self.unit.symbol(node_index) {
                    Some(Symbol::Builtin(s)) if s == "true" || s == "false" => PatternCoverage::Bool(s == "true"),
                    _ => PatternCoverage::Partial,
                })
            }
            ParenPattern => self.bind_pattern(self.unit.single_child(node_index)?, subject, initializer, from),
            ValuePattern => {
                let expr = self.unit.single_child(node_index)?;
                if let Some(Symbol::Declaration(id)) = self.unit.symbol(expr)
                    && id.kind == Case
                {
                    if pattern_subject(subject)
                        .nominal_parts()
                        .is_none_or(|(owner, _)| owner != self.catalog.case_owner[id])
                    {
                        return Err(TypeError::at(source, expr, "case does not belong to the subject"));
                    }
                    return Ok(PatternCoverage::Case(Box::new(id.clone())));
                }
                let ty = self.type_expression(expr)?;
                self.compare_value(expr, &ty, subject, initializer)?;
                Ok(match self.unit.symbol(expr) {
                    Some(Symbol::Builtin(s)) if matches!(s.as_str(), "true" | "false") => PatternCoverage::Bool(s == "true"),
                    _ => PatternCoverage::Partial,
                })
            }
            TuplePattern if matches!(subject, ValueType::Tuple(_)) => {
                let ValueType::Tuple(types) = subject else { unreachable!() };
                if types.len() != node.children.len() {
                    return Err(TypeError::at(source, node_index, "tuple pattern arity mismatch"));
                }
                let parts = node
                    .children
                    .iter()
                    .zip(types)
                    .enumerate()
                    .map(|(i, (&p, t))| self.bind_pattern(p, t, self.component(initializer, i, types.len()), &from.component(subject, i)))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                Ok(if parts.iter().all(PatternCoverage::is_total) {
                    PatternCoverage::All
                } else {
                    PatternCoverage::Product(parts)
                })
            }
            RecordPattern | TuplePattern => {
                if matches!(subject, ValueType::Reference(crate::Access::Write, _)) {
                    return Err(TypeError::at(source, node_index, "cannot read through a write-only reference"));
                }
                let (owner, args) = pattern_subject(subject)
                    .nominal_parts()
                    .ok_or_else(|| TypeError::at(source, node_index, "pattern requires a record or case subject"))?;
                let mut parts = node.children.iter().copied();
                let shape = if node.kind == RecordPattern {
                    let head = parts
                        .next()
                        .ok_or_else(|| TypeError::unsupported(source, node_index))?;
                    match self.unit.symbol(head) {
                        Some(Symbol::Declaration(id)) if id.kind == Case && self.catalog.case_owner[id] == owner => id,
                        Some(Symbol::Declaration(id)) if id == owner && !self.catalog.cases.contains_key(id) => owner,
                        _ => return Err(TypeError::at(source, head, "pattern does not match the subject type")),
                    }
                } else {
                    if self.catalog.cases.contains_key(owner) {
                        return Err(TypeError::at(source, node_index, "sum pattern requires a case"));
                    }
                    owner
                };
                let fields = &self.catalog.fields[shape];
                let order = &self.catalog.field_order[shape];
                let mut seen = HashSet::new();
                let mut rest = false;
                let mut total = true;
                for (i, part) in parts.enumerate() {
                    let p = &source.nodes[part];
                    if p.kind == RestPattern {
                        rest = true;
                        continue;
                    }
                    if node.kind == TuplePattern {
                        let name = order
                            .get(i)
                            .ok_or_else(|| TypeError::at(source, part, "too many pattern components"))?;
                        let ty = self.catalog.types.field_type(&fields[name], owner, args);
                        total &= self
                            .bind_pattern(part, &ty, part, &from.component(subject, i))?
                            .is_total();
                        seen.insert(name.as_str());
                        continue;
                    }
                    let name = self.unit.member_name(part)?;
                    let field = fields
                        .get(name)
                        .ok_or_else(|| TypeError::at(source, part, format!("unknown pattern field {}", name)))?;
                    let ty = self.catalog.types.field_type(field, owner, args);
                    if !seen.insert(name) {
                        return Err(TypeError::at(source, part, format!("duplicate pattern field {}", name)));
                    }
                    if let [child] = *p.children.as_slice() {
                        let v = self.type_expression(child)?;
                        self.compare_value(child, &v, &ty, part)?;
                        total = false;
                    } else if let Some(binding) = p.words().next_back().filter(|w| w.text != "_") {
                        self.locals.insert(binding.span, ty);
                        let position = order
                            .iter()
                            .position(|field| field == name)
                            .unwrap_or(usize::MAX);
                        self.bind_source(binding.span, &from.component(subject, position));
                    }
                }
                if !rest && seen.len() != fields.len() {
                    return Err(TypeError::at(source, node_index, "pattern omits fields without .."));
                }
                Ok(if !total {
                    PatternCoverage::Partial
                } else if shape == owner {
                    PatternCoverage::All
                } else {
                    PatternCoverage::Case(Box::new(shape.clone()))
                })
            }
            _ => Err(TypeError::unsupported(source, node_index)),
        }
    }
}
fn pattern_subject(ty: &ValueType) -> &ValueType {
    match ty {
        ValueType::Reference(_, inner) => inner,
        _ => ty,
    }
}
fn product_covers(rows: &[Vec<PatternCoverage>]) -> bool {
    if rows.is_empty() {
        return false;
    }
    // Split only the Boolean positions actually constrained by a surviving arm.
    let mut pending = vec![(0, rows.iter().collect::<Vec<_>>())];
    while let Some((index, rows)) = pending.pop() {
        if rows.is_empty() {
            return false;
        }
        if rows
            .iter()
            .any(|r| r[index..].iter().all(PatternCoverage::is_total))
        {
            continue;
        }
        // Advance through every position in order. A wildcard participates in both branches.
        if rows.iter().all(|r| r[index].is_total()) {
            pending.push((index + 1, rows));
            continue;
        }
        if rows
            .iter()
            .any(|r| !matches!(r[index], PatternCoverage::All | PatternCoverage::Bool(_)))
        {
            return false;
        }
        for value in [false, true] {
            pending.push((
                index + 1,
                rows.iter()
                    .copied()
                    .filter(|r| r[index].is_total() || matches!(r[index],PatternCoverage::Bool(b) if b==value))
                    .collect(),
            ));
        }
    }
    true
}
