use super::*;

impl Body<'_, '_> {
    pub(super) fn cast(
        &mut self,
        n: usize,
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let f = &self.unit.file;
        let (operand, rest) = f.nodes[n]
            .children
            .split_first()
            .ok_or_else(|| TypeError::unsupported(f, n))?;
        let source = self.expression(*operand)?;
        let domain_only = rest
            .first()
            .is_some_and(|&i| f.nodes[i].kind == NodeKind::CastEntry);
        let (carrier, entries) = if domain_only {
            let target = if matches!(source, ValueType::Exact { .. }) {
                let d = self.unit.domain(self.unit.single_child(rest[0])?)?;
                expected
                    .map(ValueType::carrier)
                    .unwrap_or_else(|| self.catalog.types.domains.carrier(d))
                    .clone()
            } else {
                source.carrier().clone()
            };
            (target, rest)
        } else {
            let (target, entries) = rest.split_first().ok_or_else(|| TypeError::unsupported(f, n))?;
            if !source.domains().is_empty() {
                return Err(TypeError::at(f, *operand, "carrier cast requires an unqualified operand"));
            }
            (self.unit.annotation(*target, &self.catalog.types)?, entries)
        };
        if matches!(source, ValueType::Exact { .. }) {
            self.land(*operand, &source, &carrier, true)?;
        } else if source.carrier() != &carrier {
            match (&source, &carrier) {
                (ValueType::Reference(a, _), ValueType::Reference(b, _)) if a == b => {
                    self.obligation(
                        n,
                        ObligationKind::Recast {
                            source: source.clone(),
                            destination: carrier.clone(),
                        },
                    );
                }
                _ if source.numeric() && carrier.numeric() => self.obligation(
                    n,
                    ObligationKind::ExactCast {
                        source: source.clone(),
                        destination: carrier.clone(),
                    },
                ),
                _ => return Err(TypeError::at(f, n, format!("cannot cast {source} to {carrier}"))),
            }
        }
        let mut mentioned = HashSet::new();
        let mut result = Vec::new();
        for &entry in entries {
            let domain = self
                .unit
                .domain_application(self.unit.single_child(entry)?, &self.catalog.types)?;
            let action = f.nodes[entry].leaves[0].text;
            for atom in self
                .catalog
                .types
                .domains
                .expanded(&domain, &carrier, false)
                .map_err(|m| TypeError::at(f, entry, m))?
            {
                if !mentioned.insert(atom.clone()) {
                    return Err(TypeError::at(f, entry, format!("repeated domain {}", atom.declaration.path)));
                }
                if source.domains().contains(&atom) == (action == "+") {
                    return Err(TypeError::at(f, entry, format!("invalid {action} entry for domain {}", atom.declaration.path)));
                }
                if action == "+" {
                    self.obligation(n, ObligationKind::EstablishDomain(atom.clone()));
                }
                if action != "-" {
                    result.push(atom);
                }
            }
        }
        if let Some(missing) = source.domains().iter().find(|d| !mentioned.contains(*d)) {
            return Err(TypeError::at(f, n, format!("cast omits domain {}", missing.declaration.path)));
        }
        for atom in &result {
            for parent in self
                .catalog
                .types
                .domains
                .expanded(atom, &carrier, true)
                .map_err(|m| TypeError::at(f, n, m))?
            {
                if !result.contains(&parent) {
                    return Err(TypeError::at(f, n, format!("cast omits ancestor {}", parent.declaration.path)));
                }
            }
        }
        let target = crate::domains::qualified(carrier, result);
        self.record_operator(n, "as", vec![source], &target);
        Ok(target)
    }
    pub(super) fn require_writable(
        &self,
        mut n: usize,
    ) -> Result<(), TypeError> {
        let f = &self.unit.file;
        let site = n;
        while matches!(f.nodes[n].kind, NodeKind::FieldAccess | NodeKind::Paren) {
            n = self.unit.single_child(n)?;
            if f.nodes[site].kind == NodeKind::FieldAccess
                && let Some(expr) = self
                    .output
                    .expressions
                    .iter()
                    .rev()
                    .find(|e| e.site.span == f.span(n) && e.site.package == f.package && e.site.file == f.path)
            {
                let receiver = match &expr.ty {
                    ValueType::AtLifetime(t, _) => t.as_ref(),
                    other => other,
                };
                match receiver {
                    ValueType::Reference(Access::Mut | Access::Write, _) => return Ok(()),
                    ValueType::Reference(Access::Shared, _) => return Err(TypeError::at(f, site, "place requires writable authority")),
                    _ => {}
                }
            }
        }
        if let Some(Symbol::Binding { span, .. }) = self.unit.symbol(n)
            && self.writable.contains(span)
        {
            return Ok(());
        }
        Err(TypeError::at(f, site, "place requires writable authority"))
    }
    pub(super) fn assign(
        &mut self,
        n: usize,
    ) -> Result<(), TypeError> {
        let f = &self.unit.file;
        let [place, rhs] = *f.nodes[n].children.as_slice() else {
            return Err(TypeError::unsupported(f, n));
        };
        let ty = if f.nodes[place].kind == NodeKind::FieldAccess {
            let ty = self.project_fields(place, true)?;
            self.require_writable(place)?;
            ty
        } else if f.nodes[place].kind == NodeKind::Index {
            let view = self.index(place, Some(Access::Mut))?;
            let ValueType::Reference(Access::Mut | Access::Write, inner) = view else {
                return Err(TypeError::at(f, place, "index does not return writable storage"));
            };
            *inner
        } else {
            let ty = self.expression(place)?;
            self.require_writable(place)?;
            ty
        };
        let value = self.expected(rhs, Some(&ty))?;
        self.version = Some(Site::at(f, n));
        self.cases.clear();
        self.destination(rhs, &value, &ty)?;
        if let Some(Symbol::Binding { span, .. }) = self.unit.symbol(place) {
            if self.local_view(rhs) {
                self.local_views.insert(*span);
            } else {
                self.local_views.remove(span);
            }
        }
        Ok(())
    }
    pub(super) fn case_type(
        &self,
        n: usize,
        case: &omega_resolver::DeclarationId,
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let owner = self.catalog.case_owner[case];
        if !self.unit.file.nodes[n].children.is_empty() {
            return self.unit.data_application(n, owner, &self.catalog.types, None);
        }
        if matches!(self.catalog.types.open_data(owner), ValueType::Nominal(_)) {
            return Ok(ValueType::Nominal(Box::new(owner.clone())));
        }
        expected
            .filter(|ty| ty.nominal_parts().is_some_and(|(id, _)| id == owner))
            .cloned()
            .ok_or_else(|| TypeError::at(&self.unit.file, n, "generic case needs its carrier application"))
    }
    pub(super) fn record(
        &mut self,
        n: usize,
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let f = &self.unit.file;
        let (head, inits) = f.nodes[n]
            .children
            .split_first()
            .ok_or_else(|| TypeError::unsupported(f, n))?;
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
            .ok_or_else(|| TypeError::at(f, *head, "record literal requires a record type"))?;
        if case.is_none() && self.catalog.cases.contains_key(owner) {
            return Err(TypeError::at(f, *head, "sum construction requires a case"));
        }
        let id = case.unwrap_or(owner);
        let fields = self
            .catalog
            .fields
            .get(id)
            .ok_or_else(|| TypeError::at(f, *head, "missing field declarations"))?;
        let mut seen = HashSet::new();
        let mut witnesses = vec![None; self.catalog.types.witnesses.get(owner).copied().unwrap_or(0)];
        let mut omission = false;
        for &init in inits {
            if f.nodes[init].kind == NodeKind::Omission {
                omission = true;
                continue;
            }
            if f.nodes[init].kind != NodeKind::FieldInit {
                return Err(TypeError::unsupported(f, init));
            }
            let name = f.nodes[init]
                .words()
                .next()
                .ok_or_else(|| TypeError::unsupported(f, init))?
                .text;
            let declared = fields
                .get(name)
                .ok_or_else(|| TypeError::at(f, init, format!("{} has no field {name}", id.path)))?;
            let target = self.catalog.types.field_type(declared, owner, args);
            if !seen.insert(name) {
                return Err(TypeError::at(f, init, format!("duplicate field {name}")));
            }
            let (site, value) = if f.nodes[init].children.is_empty() {
                let Some(Symbol::Binding { span, .. }) = self.unit.symbol(init) else {
                    return Err(TypeError::at(f, init, "missing shorthand binding"));
                };
                (
                    init,
                    self.locals
                        .get(span)
                        .cloned()
                        .ok_or_else(|| TypeError::at(f, init, "missing type of shorthand binding"))?,
                )
            } else {
                let expr = self.unit.single_child(init)?;
                (expr, self.expected(expr, Some(&target))?)
            };
            let target = packages::select(&target, &value, owner, &mut witnesses).ok_or_else(|| TypeError::at(f, site, "inconsistent package witness"))?;
            self.destination(site, &value, &target)?;
        }
        if !witnesses.is_empty() {
            let witnesses = witnesses
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| TypeError::at(f, n, "cannot infer every package witness"))?;
            self.output.packages.push(crate::PackageConstruction {
                site: Site::at(f, n),
                carrier: ty.clone(),
                witnesses,
            });
        }
        for name in &self.catalog.field_order[id] {
            if !seen.contains(name.as_str()) {
                if !omission {
                    return Err(TypeError::at(f, n, format!("missing field {name}")));
                }
                self.obligation(n, ObligationKind::ZeroField(id.clone(), name.clone()));
            }
        }
        if self.catalog.predicates.contains(owner) {
            self.obligation(n, ObligationKind::EstablishData(owner.clone()));
        }
        Ok(ty)
    }
    pub(super) fn field_chain(
        &mut self,
        n: usize,
    ) -> Result<ValueType, TypeError> {
        self.project_fields(n, false)
    }
    pub(super) fn project_fields(
        &mut self,
        mut n: usize,
        write: bool,
    ) -> Result<ValueType, TypeError> {
        let destination = n;
        let f = &self.unit.file;
        // Postfix selectors form arbitrarily long, left-nested trees in L1.
        let mut selectors = Vec::new();
        while f.nodes[n].kind == NodeKind::FieldAccess {
            selectors.push(n);
            n = self.unit.single_child(n)?;
        }
        let mut ty = self.expression(n)?;
        let mut opening = self.opening(n);
        let mut case = self.fact_subject(n).and_then(|s| self.cases.get(&s)).cloned();
        for n in selectors.into_iter().rev() {
            let receiver = match &ty {
                ValueType::AtLifetime(inner, _) => inner.as_ref(),
                other => other,
            };
            let receiver = match receiver {
                ValueType::Reference(Access::Write, _) if !write || n != destination => {
                    return Err(TypeError::at(f, n, "cannot read through a write-only reference"));
                }
                ValueType::Reference(_, inner) => inner.as_ref(),
                other => other,
            };
            if matches!(receiver, ValueType::Slice(_) | ValueType::Array(_, _) | ValueType::IndexedArray(..))
                && f.nodes[n]
                    .words()
                    .next()
                    .is_some_and(|w| matches!(w.text, "len" | "address"))
            {
                if write {
                    return Err(TypeError::at(f, n, "collection metadata is read-only"));
                }
                let address = f.nodes[n].words().next().is_some_and(|w| w.text == "address");
                ty = if address { ValueType::Address } else { ValueType::integer("u64").unwrap() };
                self.output.expressions.push(TypedExpression {
                    site: Site::at(f, n),
                    ty: ty.clone(),
                });
                continue;
            }
            let Some((id, args)) = receiver.nominal_parts() else {
                return Err(TypeError::at(f, n, format!("cannot select a field of {ty}")));
            };
            let field = f.nodes[n]
                .words()
                .next()
                .ok_or_else(|| TypeError::unsupported(f, n))?;
            ty = self
                .catalog
                .fields
                .get(case.take().as_ref().unwrap_or(id))
                .and_then(|fields| fields.get(field.text))
                .map(|t| packages::open(&self.catalog.types.field_type(t, id, args), id, &opening))
                .ok_or_else(|| TypeError::at(f, n, format!("{} has no field {}", id.path, field.text)))?;
            opening.fields.push(field.text.into());
            self.output.expressions.push(TypedExpression {
                site: Site::at(f, n),
                ty: ty.clone(),
            });
        }
        Ok(ty)
    }
}
