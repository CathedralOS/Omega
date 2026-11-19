use crate::expressions::body_typer::BodyTyper;
use crate::type_system::domains;
use crate::{Access, ObligationKind, Site, TypeError, TypedExpression, ValueType};
use omega_parser::NodeKind;
use omega_resolver::Symbol;
use std::collections::HashSet;

impl BodyTyper<'_, '_> {
    pub(super) fn cast(
        &mut self,
        node_index: usize,
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let source_file = &self.unit.file;
        let (operand, rest) = source_file.nodes[node_index]
            .children
            .split_first()
            .ok_or_else(|| TypeError::unsupported(source_file, node_index))?;
        let source = self.type_expression(*operand)?;
        let domain_only = rest
            .first()
            .is_some_and(|&i| source_file.nodes[i].kind == NodeKind::CastEntry);
        let (carrier, entries) = if domain_only {
            let target = if matches!(source, ValueType::Exact { .. }) {
                let path = self.unit.single_child(rest[0])?;
                let d = self.unit.domain(path)?;
                match expected {
                    Some(ty) => ty.carrier().clone(),
                    None => self
                        .unit
                        .explicit_domain_carrier(path, self.catalog.types.domains.carrier(d).carrier(), &self.catalog.types, None)?,
                }
            } else {
                source.carrier().clone()
            };
            (target, rest)
        } else {
            let (target, entries) = rest
                .split_first()
                .ok_or_else(|| TypeError::unsupported(source_file, node_index))?;
            if !source.domains().is_empty() {
                return Err(TypeError::at(source_file, *operand, "carrier cast requires an unqualified operand"));
            }
            (self.unit.annotation(*target, &self.catalog.types)?, entries)
        };
        if matches!(source, ValueType::Exact { .. }) {
            self.land_value(*operand, &source, &carrier, true)?;
        } else if source.carrier() != &carrier {
            match (&source, &carrier) {
                (ValueType::Reference(a, _), ValueType::Reference(b, _)) if a == b => {
                    self.record_obligation(
                        node_index,
                        ObligationKind::Recast {
                            source: source.clone(),
                            destination: carrier.clone(),
                        },
                    );
                }
                _ if source.numeric() && carrier.numeric() => self.record_obligation(
                    node_index,
                    ObligationKind::ExactCast {
                        source: source.clone(),
                        destination: carrier.clone(),
                    },
                ),
                _ => return Err(TypeError::at(source_file, node_index, format!("cannot cast {source} to {carrier}"))),
            }
        }
        let mut mentioned = HashSet::new();
        let mut result = Vec::new();
        for &entry in entries {
            let path = self.unit.single_child(entry)?;
            let domain = self
                .unit
                .domain_application(path, &carrier, &self.catalog.types)?;
            let action = source_file.nodes[entry].leaves[0].text;
            for atom in self
                .catalog
                .types
                .expanded_domain_at(&domain, &carrier, false, source_file, entry)
                .map_err(|m| TypeError::at(source_file, entry, m))?
            {
                if !mentioned.insert(atom.clone()) {
                    return Err(TypeError::at(source_file, entry, format!("repeated domain {}", atom.declaration.path)));
                }
                if source.domains().contains(&atom) == (action == "+") {
                    return Err(TypeError::at(
                        source_file,
                        entry,
                        format!("invalid {action} entry for domain {}", atom.declaration.path),
                    ));
                }
                if action == "+" {
                    self.record_obligation(node_index, ObligationKind::EstablishDomain(atom.clone()));
                }
                if action != "-" {
                    result.push(atom);
                }
            }
        }
        if let Some(missing) = source.domains().iter().find(|d| !mentioned.contains(*d)) {
            return Err(TypeError::at(
                source_file,
                node_index,
                format!("cast omits domain {}", missing.declaration.path),
            ));
        }
        for atom in &result {
            for parent in self
                .catalog
                .types
                .expanded_domain_at(atom, &carrier, true, source_file, node_index)
                .map_err(|m| TypeError::at(source_file, node_index, m))?
            {
                if !result.contains(&parent) {
                    return Err(TypeError::at(
                        source_file,
                        node_index,
                        format!("cast omits ancestor {}", parent.declaration.path),
                    ));
                }
            }
        }
        let target = domains::qualified(carrier, result);
        self.record_operator(node_index, "as", vec![source], &target);
        Ok(target)
    }
    pub(super) fn ungrouped_place(
        &self,
        mut node_index: usize,
    ) -> Result<usize, TypeError> {
        while self.unit.file.nodes[node_index].kind == NodeKind::Paren {
            node_index = self.unit.single_child(node_index)?;
        }
        Ok(node_index)
    }
    pub(super) fn require_writable(
        &self,
        mut node_index: usize,
    ) -> Result<(), TypeError> {
        let source = &self.unit.file;
        let site = node_index;
        while matches!(source.nodes[node_index].kind, NodeKind::FieldAccess | NodeKind::Paren) {
            let selects_field = source.nodes[node_index].kind == NodeKind::FieldAccess;
            node_index = self.unit.single_child(node_index)?;
            if selects_field
                && let Some(expr) = self
                    .output
                    .expressions
                    .iter()
                    .rev()
                    .find(|e| e.site.span == source.span(node_index) && e.site.package == source.package && e.site.file == source.path)
            {
                let receiver = match &expr.ty {
                    ValueType::AtLifetime(t, _) => t.as_ref(),
                    other => other,
                };
                match receiver {
                    ValueType::Reference(Access::Mut | Access::Write, _) => return Ok(()),
                    ValueType::Reference(Access::Shared, _) => return Err(TypeError::at(source, site, "place requires writable authority")),
                    _ => {}
                }
            }
        }
        if let Some(Symbol::Binding { span, .. }) = self.unit.symbol(node_index)
            && self.writable.contains(span)
        {
            return Ok(());
        }
        Err(TypeError::at(source, site, "place requires writable authority"))
    }
    pub(super) fn assign(
        &mut self,
        node_index: usize,
    ) -> Result<(), TypeError> {
        let source = &self.unit.file;
        let [place, right_type] = *source.nodes[node_index].children.as_slice() else {
            return Err(TypeError::unsupported(source, node_index));
        };
        let place = self.ungrouped_place(place)?;
        let ty = if source.nodes[place].kind == NodeKind::FieldAccess {
            let ty = self.project_fields(place, &Access::Write)?;
            self.require_writable(place)?;
            ty
        } else if source.nodes[place].kind == NodeKind::Index {
            let view = self.index(place, Some(Access::Mut))?;
            let ValueType::Reference(Access::Mut | Access::Write, inner) = view else {
                return Err(TypeError::at(source, place, "index does not return writable storage"));
            };
            *inner
        } else {
            let ty = self.type_expression(place)?;
            self.require_writable(place)?;
            ty
        };
        let value = self.type_expression_with_expected_type(right_type, Some(&ty))?;
        self.version = Some(Site::at(source, node_index));
        self.cases.clear();
        self.check_destination(right_type, &value, &ty)?;
        if let Some(Symbol::Binding { span, .. }) = self.unit.symbol(place) {
            if self.local_view(right_type) {
                self.local_views.insert(*span);
            } else {
                self.local_views.remove(span);
            }
        }
        Ok(())
    }
    pub(super) fn case_type(
        &self,
        node_index: usize,
        case: &omega_resolver::DeclarationId,
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let owner = self.catalog.case_owner[case];
        if !self.unit.file.nodes[node_index].children.is_empty() {
            let data = self
                .unit
                .data_application(node_index, owner, &self.catalog.types, None)?;
            return self
                .catalog
                .types
                .normalize_at(data, &self.unit.file, node_index)
                .map_err(|message| TypeError::at(&self.unit.file, node_index, message));
        }
        if matches!(self.catalog.types.open_data(owner), ValueType::Nominal(_)) {
            return Ok(ValueType::Nominal(Box::new(owner.clone())));
        }
        expected
            .filter(|ty| ty.nominal_parts().is_some_and(|(id, _)| id == owner))
            .cloned()
            .ok_or_else(|| TypeError::at(&self.unit.file, node_index, "generic case needs its carrier application"))
    }
    pub(super) fn record(
        &mut self,
        node_index: usize,
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let source = &self.unit.file;
        let (head, inits) = source.nodes[node_index]
            .children
            .split_first()
            .ok_or_else(|| TypeError::unsupported(source, node_index))?;
        let case = match self.unit.symbol(*head) {
            Some(Symbol::Declaration(id)) if id.kind == NodeKind::Case => Some(id),
            _ => None,
        };
        let ty = if let Some(id) = case {
            self.case_type(*head, id, expected)?
        } else {
            self.unit.annotation(*head, &self.catalog.types)?
        };
        let (owner, args) = ty
            .nominal_parts()
            .ok_or_else(|| TypeError::at(source, *head, "record literal requires a record type"))?;
        if case.is_none() && self.catalog.cases.contains_key(owner) {
            return Err(TypeError::at(source, *head, "sum construction requires a case"));
        }
        let id = case.unwrap_or(owner);
        let fields = self
            .catalog
            .fields
            .get(id)
            .ok_or_else(|| TypeError::at(source, *head, "missing field declarations"))?;
        let mut seen = HashSet::new();
        let mut witnesses = vec![None; self.catalog.types.witnesses.get(owner).copied().unwrap_or(0)];
        let mut omission = false;
        for &init in inits {
            if source.nodes[init].kind == NodeKind::Omission {
                omission = true;
                continue;
            }
            if source.nodes[init].kind != NodeKind::FieldInit {
                return Err(TypeError::unsupported(source, init));
            }
            let name = source.nodes[init]
                .words()
                .next()
                .ok_or_else(|| TypeError::unsupported(source, init))?
                .text;
            let declared = fields
                .get(name)
                .ok_or_else(|| TypeError::at(source, init, format!("{} has no field {name}", id.path)))?;
            let target = self.catalog.types.field_type(declared, owner, args);
            if !seen.insert(name) {
                return Err(TypeError::at(source, init, format!("duplicate field {name}")));
            }
            let (site, value) = if source.nodes[init].children.is_empty() {
                let Some(Symbol::Binding { span, .. }) = self.unit.symbol(init) else {
                    return Err(TypeError::at(source, init, "missing shorthand binding"));
                };
                (
                    init,
                    self.locals
                        .get(span)
                        .cloned()
                        .ok_or_else(|| TypeError::at(source, init, "missing type of shorthand binding"))?,
                )
            } else {
                let expr = self.unit.single_child(init)?;
                (expr, self.type_expression_with_expected_type(expr, Some(&target))?)
            };
            let target = super::existential_records::select(&target, &value, owner, &mut witnesses)
                .ok_or_else(|| TypeError::at(source, site, "inconsistent package witness"))?;
            self.check_destination(site, &value, &target)?;
        }
        if !witnesses.is_empty() {
            let witnesses = witnesses
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| TypeError::at(source, node_index, "cannot infer every package witness"))?;
            self.output.packages.push(crate::PackageConstruction {
                site: Site::at(source, node_index),
                carrier: ty.clone(),
                witnesses,
            });
        }
        for name in &self.catalog.field_order[id] {
            if !seen.contains(name.as_str()) {
                if !omission {
                    return Err(TypeError::at(source, node_index, format!("missing field {name}")));
                }
                self.record_obligation(node_index, ObligationKind::ZeroField(id.clone(), name.clone()));
            }
        }
        if self.catalog.predicates.contains(owner) {
            self.record_obligation(node_index, ObligationKind::EstablishData(owner.clone()));
        }
        Ok(ty)
    }
    pub(super) fn field_chain(
        &mut self,
        node_index: usize,
    ) -> Result<ValueType, TypeError> {
        self.project_fields(node_index, &Access::Shared)
    }
    pub(super) fn project_fields(
        &mut self,
        mut node_index: usize,
        access: &Access,
    ) -> Result<ValueType, TypeError> {
        let destination = node_index;
        let source = &self.unit.file;
        // Postfix selectors form arbitrarily long, left-nested trees in L1.
        let mut selectors = Vec::new();
        while source.nodes[node_index].kind == NodeKind::FieldAccess {
            selectors.push(node_index);
            node_index = self.unit.single_child(node_index)?;
        }
        let mut ty = self.type_expression(node_index)?;
        let mut opening = self.opening(node_index);
        let mut case = self
            .fact_subject(node_index)
            .and_then(|s| self.cases.get(&s))
            .cloned();
        for node_index in selectors.into_iter().rev() {
            let receiver = match &ty {
                ValueType::AtLifetime(inner, _) => inner.as_ref(),
                other => other,
            };
            let receiver = match receiver {
                ValueType::Reference(Access::Write, _) if *access != Access::Write || node_index != destination => {
                    return Err(TypeError::at(source, node_index, "cannot read through a write-only reference"));
                }
                ValueType::Reference(_, inner) => inner.as_ref(),
                other => other,
            };
            if matches!(receiver, ValueType::Slice(_) | ValueType::Array(_, _) | ValueType::IndexedArray(..))
                && source.nodes[node_index]
                    .words()
                    .next()
                    .is_some_and(|w| matches!(w.text, "len" | "address"))
            {
                if *access != Access::Shared {
                    return Err(TypeError::at(source, node_index, "collection metadata is read-only"));
                }
                let address = source.nodes[node_index]
                    .words()
                    .next()
                    .is_some_and(|w| w.text == "address");
                ty = if address { ValueType::Address } else { ValueType::integer("u64").unwrap() };
                self.output.expressions.push(TypedExpression {
                    site: Site::at(source, node_index),
                    ty: ty.clone(),
                });
                continue;
            }
            let Some((id, args)) = receiver.nominal_parts() else {
                return Err(TypeError::at(source, node_index, format!("cannot select a field of {ty}")));
            };
            let field = source.nodes[node_index]
                .words()
                .next()
                .ok_or_else(|| TypeError::unsupported(source, node_index))?;
            ty = self
                .catalog
                .fields
                .get(case.take().as_ref().unwrap_or(id))
                .and_then(|fields| fields.get(field.text))
                .map(|t| super::existential_records::open(&self.catalog.types.field_type(t, id, args), id, &opening))
                .ok_or_else(|| TypeError::at(source, node_index, format!("{} has no field {}", id.path, field.text)))?;
            opening.fields.push(field.text.into());
            self.output.expressions.push(TypedExpression {
                site: Site::at(source, node_index),
                ty: ty.clone(),
            });
        }
        Ok(ty)
    }
}
