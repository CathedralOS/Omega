use super::{Body, TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};
use std::collections::HashSet;

pub(super) enum Cover {
    All,
    Bool(bool),
    Case(Box<DeclarationId>),
    Partial,
    Product(Vec<Cover>),
}
impl Cover {
    pub(super) fn is_total(&self) -> bool {
        matches!(self, Self::All)
    }
}

impl Body<'_, '_> {
    pub(super) fn dispatch(
        &mut self,
        n: usize,
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let f = &self.unit.file;
        let mut parts = f.nodes[n].children.iter().copied().peekable();
        let subject_place = parts.peek().and_then(|&i| self.fact_subject(i));
        let subject = if parts.peek().is_some_and(|&i| f.nodes[i].kind != NodeKind::Arm) {
            Some(self.expression(parts.next().unwrap())?)
        } else {
            None
        };
        let mut all = false;
        let mut booleans = [false; 2];
        let mut cases = HashSet::new();
        let mut values = Vec::new();
        let mut products = Vec::new();
        let entry_cases = self.cases.clone();
        let entry_views = self.local_views.clone();
        let entry_version = self.version.clone();
        let mut changed = false;
        let mut merged_views = HashSet::new();
        for arm in parts {
            self.cases.clone_from(&entry_cases);
            self.local_views.clone_from(&entry_views);
            self.version.clone_from(&entry_version);
            let [pattern, expr] = *f.nodes[arm].children.as_slice() else {
                return Err(TypeError::unsupported(f, arm));
            };
            let cover = if let Some(ty) = &subject {
                self.bind_pattern(pattern, ty)?
            } else {
                self.guard(pattern)?
            };
            match cover {
                Cover::All => all = true,
                Cover::Bool(b) => booleans[usize::from(b)] = true,
                Cover::Case(id) => {
                    if let Some(subject) = subject_place {
                        self.cases.insert(subject, *id.clone());
                    }
                    cases.insert(*id);
                }
                Cover::Product(parts) => products.push(parts),
                Cover::Partial => {}
            }
            values.push((expr, self.expected(expr, expected)?));
            changed |= self.version != entry_version;
            merged_views.extend(&self.local_views);
        }
        self.local_views = merged_views;
        self.version = if changed { Some(super::Site::at(f, n)) } else { entry_version };
        self.cases = if changed { Default::default() } else { entry_cases };
        let subject = subject.as_ref().map(pattern_subject);
        let complete_sum = subject
            .and_then(ValueType::nominal_parts)
            .and_then(|(id, _)| self.catalog.cases.get(id))
            .is_some_and(|ids| ids.iter().all(|id| cases.contains(*id)));
        if !(all || product_covers(&products) || subject == Some(&ValueType::Bool) && booleans == [true, true] || complete_sum) {
            return Err(TypeError::at(f, n, "dispatch does not cover its subject"));
        }
        let target = expected
            .cloned()
            .or_else(|| {
                values
                    .iter()
                    .find(|(_, t)| !matches!(t, ValueType::Exact { .. }))
                    .map(|(_, t)| t.clone())
            })
            .ok_or_else(|| TypeError::at(f, n, "dispatch needs a result type"))?;
        for (expr, value) in values {
            self.destination(expr, &value, &target)?;
        }
        Ok(target)
    }
    fn guard(
        &mut self,
        n: usize,
    ) -> Result<Cover, TypeError> {
        let f = &self.unit.file;
        match f.nodes[n].kind {
            NodeKind::WildcardPattern => Ok(Cover::All),
            NodeKind::ParenPattern => self.guard(self.unit.single_child(n)?),
            NodeKind::ValuePattern => {
                let expr = self.unit.single_child(n)?;
                let value = self.expression(expr)?;
                self.destination(expr, &value, &ValueType::Bool)?;
                Ok(Cover::Partial)
            }
            NodeKind::BindingPattern => {
                let ty = self.pattern_value(n)?;
                self.destination(n, &ty, &ValueType::Bool)?;
                Ok(if matches!(self.unit.symbol(n),Some(Symbol::Builtin(s)) if s=="true") {
                    Cover::All
                } else {
                    Cover::Partial
                })
            }
            _ => Err(TypeError::unsupported(f, n)),
        }
    }
    fn pattern_value(
        &self,
        n: usize,
    ) -> Result<ValueType, TypeError> {
        match self.unit.symbol(n) {
            Some(Symbol::Builtin(s)) if matches!(s.as_str(), "true" | "false") => Ok(ValueType::Bool),
            Some(Symbol::Binding { span, .. }) => self
                .locals
                .get(span)
                .cloned()
                .ok_or_else(|| TypeError::unsupported(&self.unit.file, n)),
            Some(Symbol::Declaration(id)) if id.kind == NodeKind::Const => Ok(self.catalog.constants[id].clone()),
            _ => Err(TypeError::unsupported(&self.unit.file, n)),
        }
    }
    pub(super) fn bind_pattern(
        &mut self,
        n: usize,
        subject: &ValueType,
    ) -> Result<Cover, TypeError> {
        use NodeKind::*;
        let f = &self.unit.file;
        let node = &f.nodes[n];
        match node.kind {
            WildcardPattern => Ok(Cover::All),
            BindingPattern if self.unit.symbol(n).is_none() => {
                self.locals.insert(f.span(n), subject.clone());
                Ok(Cover::All)
            }
            BindingPattern => {
                let ty = self.pattern_value(n)?;
                self.destination(n, &ty, subject)?;
                Ok(match self.unit.symbol(n) {
                    Some(Symbol::Builtin(s)) if s == "true" || s == "false" => Cover::Bool(s == "true"),
                    _ => Cover::Partial,
                })
            }
            ParenPattern => self.bind_pattern(self.unit.single_child(n)?, subject),
            ValuePattern => {
                let expr = self.unit.single_child(n)?;
                if let Some(Symbol::Declaration(id)) = self.unit.symbol(expr)
                    && id.kind == Case
                {
                    if pattern_subject(subject)
                        .nominal_parts()
                        .is_none_or(|(owner, _)| owner != self.catalog.case_owner[id])
                    {
                        return Err(TypeError::at(f, expr, "case does not belong to the subject"));
                    }
                    return Ok(Cover::Case(Box::new(id.clone())));
                }
                let ty = self.expression(expr)?;
                self.destination(expr, &ty, subject)?;
                Ok(match self.unit.symbol(expr) {
                    Some(Symbol::Builtin(s)) if matches!(s.as_str(), "true" | "false") => Cover::Bool(s == "true"),
                    _ => Cover::Partial,
                })
            }
            TuplePattern if matches!(subject, ValueType::Tuple(_)) => {
                let ValueType::Tuple(types) = subject else { unreachable!() };
                if types.len() != node.children.len() {
                    return Err(TypeError::at(f, n, "tuple pattern arity mismatch"));
                }
                let parts = node
                    .children
                    .iter()
                    .zip(types)
                    .map(|(&p, t)| self.bind_pattern(p, t))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                Ok(if parts.iter().all(Cover::is_total) {
                    Cover::All
                } else {
                    Cover::Product(parts)
                })
            }
            RecordPattern | TuplePattern => {
                if matches!(subject, ValueType::Reference(super::Access::Write, _)) {
                    return Err(TypeError::at(f, n, "cannot read through a write-only reference"));
                }
                let (owner, args) = pattern_subject(subject)
                    .nominal_parts()
                    .ok_or_else(|| TypeError::at(f, n, "pattern requires a record or case subject"))?;
                let mut parts = node.children.iter().copied();
                let shape = if node.kind == RecordPattern {
                    let head = parts.next().ok_or_else(|| TypeError::unsupported(f, n))?;
                    match self.unit.symbol(head) {
                        Some(Symbol::Declaration(id)) if id.kind == Case && self.catalog.case_owner[id] == owner => id,
                        Some(Symbol::Declaration(id)) if id == owner && !self.catalog.cases.contains_key(id) => owner,
                        _ => return Err(TypeError::at(f, head, "pattern does not match the subject type")),
                    }
                } else {
                    if self.catalog.cases.contains_key(owner) {
                        return Err(TypeError::at(f, n, "sum pattern requires a case"));
                    }
                    owner
                };
                let fields = &self.catalog.fields[shape];
                let order = &self.catalog.field_order[shape];
                let mut seen = HashSet::new();
                let mut rest = false;
                let mut total = true;
                for (i, part) in parts.enumerate() {
                    let p = &f.nodes[part];
                    if p.kind == RestPattern {
                        rest = true;
                        continue;
                    }
                    if node.kind == TuplePattern {
                        let name = order
                            .get(i)
                            .ok_or_else(|| TypeError::at(f, part, "too many pattern components"))?;
                        let ty = self.catalog.types.field_type(&fields[name], owner, args);
                        total &= self.bind_pattern(part, &ty)?.is_total();
                        seen.insert(name.as_str());
                        continue;
                    }
                    let first = p
                        .words()
                        .next()
                        .ok_or_else(|| TypeError::unsupported(f, part))?;
                    let field = fields
                        .get(first.text)
                        .ok_or_else(|| TypeError::at(f, part, format!("unknown pattern field {}", first.text)))?;
                    let ty = self.catalog.types.field_type(field, owner, args);
                    if !seen.insert(first.text) {
                        return Err(TypeError::at(f, part, format!("duplicate pattern field {}", first.text)));
                    }
                    if let [child] = *p.children.as_slice() {
                        let v = self.expression(child)?;
                        self.destination(child, &v, &ty)?;
                        total = false;
                    } else if let Some(binding) = p.words().next_back().filter(|w| w.text != "_") {
                        self.locals.insert(binding.span, ty);
                    }
                }
                if !rest && seen.len() != fields.len() {
                    return Err(TypeError::at(f, n, "pattern omits fields without .."));
                }
                Ok(if !total {
                    Cover::Partial
                } else if shape == owner {
                    Cover::All
                } else {
                    Cover::Case(Box::new(shape.clone()))
                })
            }
            _ => Err(TypeError::unsupported(f, n)),
        }
    }
}
fn pattern_subject(ty: &ValueType) -> &ValueType {
    match ty {
        ValueType::Reference(_, inner) => inner,
        _ => ty,
    }
}
fn product_covers(rows: &[Vec<Cover>]) -> bool {
    if rows.is_empty() {
        return false;
    }
    // Split only the Boolean positions actually constrained by a surviving arm.
    let mut pending = vec![(0, rows.iter().collect::<Vec<_>>())];
    while let Some((index, rows)) = pending.pop() {
        if rows.is_empty() {
            return false;
        }
        if rows.iter().any(|r| r[index..].iter().all(Cover::is_total)) {
            continue;
        }
        // Advance through every position in order. A wildcard participates in both branches.
        if rows.iter().all(|r| r[index].is_total()) {
            pending.push((index + 1, rows));
            continue;
        }
        if rows
            .iter()
            .any(|r| !matches!(r[index], Cover::All | Cover::Bool(_)))
        {
            return false;
        }
        for value in [false, true] {
            pending.push((
                index + 1,
                rows.iter()
                    .copied()
                    .filter(|r| r[index].is_total() || matches!(r[index],Cover::Bool(b) if b==value))
                    .collect(),
            ));
        }
    }
    true
}
