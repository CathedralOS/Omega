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
        machine_result: bool,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        let mut parts = source.nodes[node_index].children.iter().copied().peekable();
        let subject_place = parts.peek().and_then(|&i| self.fact_subject(i));
        let mut subject_source = PatternSource::default();
        let subject = if let Some(expr) = parts.next_if(|&i| source.nodes[i].kind != NodeKind::Arm) {
            let (ty, from) = self.pattern_subject(expr)?;
            subject_source = from;
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
        let mut result_origin: Option<Origin> = None;
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
            let value = self.type_at_destination(expr, expected, machine_result)?;
            // A terminal transition returns the selected arm directly. Attach
            // its return obligation to that expression while its facts are in scope.
            if source.nodes[node_index].kind == NodeKind::Transition
                && let Some(body) = source.nodes[node_index].parent
                && let Some(result) = expected
            {
                self.check_return_borrow(body, expr, result)?;
            }
            let branch = self.origin(expr);
            result_origin = Some(result_origin.map_or_else(|| branch.clone(), |prior| prior.join(&branch)));
            values.push((expr, value));
            changed |= self.version != entry_version;
            let merged = merged_origins.get_or_insert_default();
            for (span, origin) in &self.binding_origins {
                let merged = merged.entry(*span).or_insert(Origin::Caller);
                *merged = origin.join(merged);
            }
        }
        self.binding_origins = merged_origins.unwrap_or(entry_origins);
        self.set_origin(node_index, result_origin.unwrap_or_default());
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
        from: &PatternSource,
    ) -> Result<(), TypeError> {
        let compared = if let Some((access, inner)) = subject.reference_parts() {
            let through = access
                .through(from.place.as_ref().map(|p| &p.access))
                .map_err(|m| TypeError::at(self.unit.file, pattern, m))?;
            if through == Access::Write {
                return Err(TypeError::at(self.unit.file, pattern, "cannot read through a write-only reference"));
            }
            self.land_value(read, subject, inner, false)?;
            inner
        } else {
            if let Some(place) = &from.place {
                let inspection = Access::Shared
                    .through(Some(&place.access))
                    .map_err(|m| TypeError::at(self.unit.file, read, m))?;
                self.read_borrowed(read, &inspection, subject)?;
            }
            subject
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
        let node = source.ungroup(initializer);
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
        self.bind_pattern_at(node_index, subject, initializer, from, initializer)
    }
    fn bind_pattern_at(
        &mut self,
        node_index: usize,
        subject: &ValueType,
        initializer: usize,
        from: &PatternSource,
        place: usize,
    ) -> Result<PatternCoverage, TypeError> {
        use NodeKind::*;
        let source = self.unit.file;
        let node = &source.nodes[node_index];
        match node.kind {
            WildcardPattern => Ok(PatternCoverage::All),
            BindingPattern if let Some(span) = self.unit.pattern_binding(node_index) => {
                self.require_declared_number(initializer, subject)?;
                if from.place.is_some() {
                    self.record_place(node_index, place, None, None);
                }
                self.bind_source(span, node_index, subject.clone(), from);
                Ok(PatternCoverage::All)
            }
            BindingPattern => {
                let ty = self.pattern_value(node_index)?;
                self.compare_value(node_index, &ty, subject, initializer, from)?;
                Ok(match self.unit.symbol(node_index) {
                    Some(Symbol::Builtin(s)) if s == "true" || s == "false" => PatternCoverage::Bool(s == "true"),
                    _ => PatternCoverage::Partial,
                })
            }
            ParenPattern => self.bind_pattern_at(self.unit.single_child(node_index)?, subject, initializer, from, place),
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
                self.compare_value(expr, &ty, subject, initializer, from)?;
                Ok(match self.unit.symbol(expr) {
                    Some(Symbol::Builtin(s)) if matches!(s.as_str(), "true" | "false") => PatternCoverage::Bool(s == "true"),
                    _ => PatternCoverage::Partial,
                })
            }
            TuplePattern if matches!(subject, ValueType::Tuple(_)) => {
                let ValueType::Tuple(types) = subject else { unreachable!() };
                let (patterns, rest) = match node.children.split_last() {
                    Some((&last, patterns)) if source.nodes[last].kind == RestPattern => (patterns, true),
                    _ => (node.children.as_slice(), false),
                };
                if patterns.len() > types.len() || !rest && patterns.len() != types.len() {
                    return Err(TypeError::at(source, node_index, "tuple pattern arity mismatch"));
                }
                let mut parts = patterns
                    .iter()
                    .zip(types)
                    .enumerate()
                    .map(|(i, (&p, t))| {
                        let expr = self.component(initializer, i, types.len());
                        let place = if expr == initializer {
                            self.record_place(p, place, None, Some(crate::Projection::Tuple(i)));
                            p
                        } else {
                            expr
                        };
                        let component = from
                            .component(subject, i)
                            .map_err(|m| TypeError::at(source, p, m))?;
                        self.bind_pattern_at(p, t, expr, &component, place)
                    })
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                Ok(if parts.iter().all(PatternCoverage::is_total) {
                    PatternCoverage::All
                } else {
                    // Waived positions are unconstrained in every arm that omits them.
                    parts.resize_with(types.len(), || PatternCoverage::All);
                    PatternCoverage::Product(parts)
                })
            }
            RecordPattern | TuplePattern => {
                if matches!(subject.reference_parts(), Some((crate::Access::Write, _))) {
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
                let mut seen = HashSet::new();
                let mut rest = false;
                let mut total = true;
                for (i, part) in parts.enumerate() {
                    let p = &source.nodes[part];
                    if p.kind == RestPattern {
                        rest = true;
                        continue;
                    }
                    let (position, field) = if node.kind == TuplePattern {
                        (
                            i,
                            fields
                                .ordered
                                .get(i)
                                .ok_or_else(|| TypeError::at(source, part, "too many pattern components"))?,
                        )
                    } else {
                        let name = self.unit.member_name(part)?;
                        fields
                            .get(name)
                            .ok_or_else(|| TypeError::at(source, part, format!("unknown pattern field {name}")))?
                    };
                    self.unit.require_selection_authority(part, owner)?;
                    self.unit
                        .require_selection_authority(part, &field.declaration)?;
                    let name = field.name();
                    if !seen.insert(name) {
                        return Err(TypeError::at(source, part, format!("duplicate pattern field {name}")));
                    }
                    let ty = self
                        .catalog
                        .types
                        .opened_field(&field.ty, owner, args, || self.opening(place));
                    self.record_place(
                        part,
                        place,
                        subject.reference_parts().map(|(access, _)| access),
                        Some(crate::Projection::Field(Box::new(field.declaration.clone()))),
                    );
                    let component = from
                        .component(subject, position)
                        .map_err(|m| TypeError::at(source, part, m))?;
                    if node.kind == TuplePattern {
                        total &= self
                            .bind_pattern_at(part, &ty, part, &component, part)?
                            .is_total();
                    } else if let [child] = *p.children.as_slice() {
                        let v = self.type_expression(child)?;
                        self.compare_value(child, &v, &ty, part, &component)?;
                        total = false;
                    } else if let Some(span) = self.unit.pattern_binding(part) {
                        self.bind_source(span, part, ty, &component);
                    }
                }
                if !rest && seen.len() != fields.ordered.len() {
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
    ty.reference_parts().map_or(ty, |(_, inner)| inner)
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
