use super::destination::Destination;
use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::expressions::body_typer::BodyTyper;
use crate::expressions::storage_origin::{Origin, PatternSource};
use crate::type_error::SourceTypeError;
use crate::type_relations::StoredReference;
use crate::{Access, TypeError, ValueType};
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind;
use omega_typed::site_at;
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
        destination: Destination<'_>,
        machine_result: bool,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        let expected = destination.exact();
        let mut parts = source.nodes[node_index].children.iter().copied().peekable();
        let mut subject_source = PatternSource::default();
        let subject = if let Some(expr) = parts.next_if(|&i| source.nodes[i].kind != NodeKind::Arm) {
            let (ty, from) = self.pattern_subject(expr)?;
            subject_source = from;
            Some((expr, ty))
        } else {
            None
        };
        let subject_place = subject.as_ref().and_then(|(expr, _)| self.fact_subject(*expr));
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
                    self.output
                        .complete_case_patterns
                        .insert(source.node_id(pattern), (*id).clone());
                    if let Some(subject) = &subject_place {
                        self.cases.insert(subject.clone(), *id.clone());
                    }
                    cases.insert(*id);
                }
                PatternCoverage::Product(parts) => products.push(parts),
                PatternCoverage::Partial => {}
            }
            let value = self.type_at_destination(expr, destination, machine_result)?;
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
        self.version = if changed { Some(site_at(source, node_index)) } else { entry_version };
        self.cases = if changed { Default::default() } else { entry_cases };
        let subject = subject.as_ref().map(|(_, ty)| pattern_subject(ty));
        let complete_sum = subject
            .and_then(ValueType::nominal_parts)
            .and_then(|(id, _)| self.catalog.cases.get(id))
            .is_some_and(|ids| ids.iter().all(|id| cases.contains(*id)));
        let complete_bool = subject == Some(&ValueType::Bool) && booleans == [true, true];
        if !(all || complete_bool || complete_sum || product_covers(subject, &products, self.catalog)) {
            return Err(TypeError::for_reason(&site_at(source, node_index), crate::DispatchError::UncoveredSubject));
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
        // A grouped pattern is its operand; groups nest as deeply as L1 admits.
        let mut node_index = node_index;
        while source.nodes[node_index].kind == NodeKind::ParenPattern {
            node_index = self.unit.single_child(node_index)?;
        }
        match source.nodes[node_index].kind {
            NodeKind::WildcardPattern => Ok(PatternCoverage::All),
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
        &mut self,
        node_index: usize,
    ) -> Result<ValueType, TypeError> {
        match self.unit.symbol(node_index) {
            Some(Symbol::Builtin(s)) if matches!(s.as_str(), "true" | "false") => Ok(ValueType::Bool),
            Some(Symbol::Binding { span, .. }) => self
                .locals
                .get(span)
                .cloned()
                .ok_or_else(|| TypeError::unsupported(self.unit.file, node_index)),
            Some(Symbol::Declaration(id)) if id.kind == NodeKind::Const => self.constant_reference(node_index, id),
            _ => Err(TypeError::unsupported(self.unit.file, node_index)),
        }
    }
    fn compare_value(
        &mut self,
        pattern: usize,
        value: &ValueType,
        subject: &ValueType,
        read: usize,
        from: &PatternSource,
    ) -> Result<(), TypeError> {
        self.check_destination(pattern, value, subject)?;
        if let Some(place) = &from.place {
            let inspection = Access::Shared
                .through(place.access.as_ref())
                .map_err(|m| TypeError::at(self.unit.file, read, m))?;
            self.read_borrowed(read, &inspection, subject)?;
        }
        Ok(())
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
        // A grouped pattern is its operand; groups nest as deeply as L1 admits.
        let mut node_index = node_index;
        while source.nodes[node_index].kind == ParenPattern {
            node_index = self.unit.single_child(node_index)?;
        }
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
            ValuePattern => {
                let expr = self.unit.single_child(node_index)?;
                if let Some(Symbol::Declaration(id)) = self.unit.symbol(expr)
                    && id.kind == Case
                {
                    if !from.permits_inspection(subject) {
                        return Err(TypeError::at(source, node_index, "cannot read through a write-only reference"));
                    }
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
            RecordPattern | TuplePattern if !from.permits_inspection(subject) => {
                Err(TypeError::at(source, node_index, "cannot read through a write-only reference"))
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
                let fields = self
                    .catalog
                    .fields_at(shape, source, node_index)?
                    .ok_or_else(|| TypeError::at(source, node_index, "missing field declarations"))?;
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
fn product_covers(
    subject: Option<&ValueType>,
    rows: &[Vec<PatternCoverage>],
    catalog: &DeclarationCatalog<'_>,
) -> bool {
    let Some(ValueType::Tuple(types)) = subject else { return false };
    if rows.is_empty() {
        return false;
    }
    // Expand a nested tuple only where an arm actually inspects its components.
    // A wildcard over the tuple also covers every component of that tuple.
    let mut components: Vec<_> = types
        .iter()
        .enumerate()
        .rev()
        .map(|(index, ty)| (ty, rows.iter().map(|row| &row[index]).collect::<Vec<_>>()))
        .collect();
    let mut columns = Vec::new();
    while let Some((ty, patterns)) = components.pop() {
        if let ValueType::Tuple(fields) = pattern_subject(ty)
            && patterns
                .iter()
                .any(|pattern| matches!(pattern, PatternCoverage::Product(_)))
        {
            for (index, field) in fields.iter().enumerate().rev() {
                let children = patterns
                    .iter()
                    .map(|&pattern| match pattern {
                        PatternCoverage::Product(parts) => &parts[index],
                        other => other,
                    })
                    .collect();
                components.push((field, children));
            }
        } else {
            columns.push((ty, patterns));
        }
    }

    let mut pending = vec![(0, (0..rows.len()).collect::<Vec<_>>())];
    while let Some((index, arms)) = pending.pop() {
        if arms.is_empty() {
            return false;
        }
        if arms
            .iter()
            .any(|&arm| columns[index..].iter().all(|(_, cells)| cells[arm].is_total()))
        {
            continue;
        }
        let (ty, cells) = &columns[index];
        let matching = |accepts: &dyn Fn(&PatternCoverage) -> bool| {
            arms.iter()
                .copied()
                .filter(|&arm| cells[arm].is_total() || accepts(cells[arm]))
                .collect()
        };
        if pattern_subject(ty) == &ValueType::Bool
            && arms
                .iter()
                .any(|&arm| matches!(cells[arm], PatternCoverage::Bool(_)))
        {
            for value in [false, true] {
                pending.push((index + 1, matching(&|cell| matches!(cell, PatternCoverage::Bool(b) if *b == value))));
            }
        } else if arms
            .iter()
            .any(|&arm| matches!(cells[arm], PatternCoverage::Case(_)))
            && let Some((owner, _)) = pattern_subject(ty).nominal_parts()
            && let Some(cases) = catalog.cases.get(owner)
        {
            // Enumerate the declaration's cases, including any case omitted from the arms.
            for case in cases {
                pending.push((index + 1, matching(&|cell| matches!(cell, PatternCoverage::Case(id) if id.as_ref() == *case))));
            }
        } else {
            // A value comparison or partial payload cannot prove coverage for this component.
            pending.push((index + 1, arms.into_iter().filter(|&arm| cells[arm].is_total()).collect()));
        }
    }
    true
}
