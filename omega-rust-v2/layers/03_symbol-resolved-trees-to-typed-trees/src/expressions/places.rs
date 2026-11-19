use crate::expressions::body_typer::BodyTyper;
use crate::expressions::storage_origin::{Origin, weaker};
use crate::type_system::domains;
use crate::{Access, ObligationKind, Site, TypeError, TypedExpression, ValueType};
use omega_parser::NodeKind;
use omega_resolver::Symbol;
use std::collections::HashSet;

pub(super) enum PlaceUse {
    Assignment,
    Borrow { site: usize, access: Access },
}

impl BodyTyper<'_, '_> {
    pub(super) fn cast(
        &mut self,
        node_index: usize,
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let source_file = self.unit.file;
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
    /// Assignment and explicit borrowing must select the same storage through any
    /// number of grouping nodes. Indexing selects its own operation and records
    /// the loan on the collection, so it must not also record a field or local loan.
    pub(super) fn check_place(
        &mut self,
        mut place: usize,
        usage: PlaceUse,
    ) -> Result<(usize, ValueType), TypeError> {
        use crate::Access as Authority;
        use NodeKind::*;
        let access = match &usage {
            PlaceUse::Assignment => &Authority::Write,
            PlaceUse::Borrow { access, .. } => access,
        };
        let source = self.unit.file;
        let ty = loop {
            match source.nodes[place].kind {
                Paren => place = self.unit.single_child(place)?,
                FieldAccess => break self.project_fields(place, access)?.0,
                Index => {
                    let ty = match usage {
                        PlaceUse::Assignment => {
                            let view = self.index(place, Some(Authority::Mut))?;
                            let ValueType::Reference(Authority::Mut | Authority::Write, inner) = view else {
                                return Err(TypeError::at(source, place, "index does not return writable storage"));
                            };
                            *inner
                        }
                        PlaceUse::Borrow { access, .. } => self.index(place, Some(access))?,
                    };
                    return Ok((place, ty));
                }
                PathExpression => break self.place_operand(place)?.0,
                Literal | Binary | Unary | Cast | Tuple | Move | CallMarker | Call | RecordLiteral | Match | Body | ArrayLiteral | Borrow => {
                    break self.type_expression(place)?;
                }
                File | Fragment | Use | Const | Data | Field | Case | CaseField | StableNumber | Attributes | Attribute | DroppedBy | Domain | Access
                | Trait | Restriction | Conformance | SelectionRow | Hypothesis | Machine | AsmMachine | OperatorToken | MathLet | Path | GenericParams
                | TypeParam | KindedParam | ConstParam | MachineParam | SomeParam | LifetimeParam | Params | Param | Result | Requires | Ensures | Reaches
                | Crashes | Blocks | Suspends | Terminates | Invokes | Satisfies | Registers | Clobbers | ReferenceType | SliceType | ArrayType
                | MachineType | FunctionType | FamilyApplication | PathType | TypeArgs | QualifiedType | Lifetime | Let | Assign | Discard
                | ExpressionStatement | Tail | AsmBlock | Instruction | AsmTransition | Transition | Arm | WildcardPattern | BindingPattern | RecordPattern
                | FieldPattern | RestPattern | TuplePattern | ParenPattern | ValuePattern | ExpressionArgs | FieldInit | Omission | Selection | Args
                | Range | CastEntry | Unfinished => return Err(TypeError::unsupported(source, place)),
            }
        };
        if *access != Authority::Shared {
            self.require_writable(place)?;
        }
        let result = match usage {
            PlaceUse::Assignment => ty,
            PlaceUse::Borrow { site, access } => {
                self.record_obligation(
                    site,
                    ObligationKind::Borrow {
                        access: access.clone(),
                        place: Site::at(source, place),
                    },
                );
                ValueType::Reference(access, Box::new(ty))
            }
        };
        Ok((place, result))
    }
    pub(super) fn require_writable(
        &self,
        mut node_index: usize,
    ) -> Result<(), TypeError> {
        let source = self.unit.file;
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
        let source = self.unit.file;
        let [place, right_type] = *source.nodes[node_index].children.as_slice() else {
            return Err(TypeError::unsupported(source, node_index));
        };
        let (place, ty) = self.check_place(place, PlaceUse::Assignment)?;
        let value = self.type_expression_with_expected_type(right_type, Some(&ty))?;
        self.version = Some(Site::at(source, node_index));
        self.cases.clear();
        self.check_destination(right_type, &value, &ty)?;
        // Assigning a binding replaces its value. Assigning one of its fields
        // or elements adds the stored value's loans to the binding's.
        let mut root = place;
        while matches!(source.nodes[root].kind, NodeKind::FieldAccess | NodeKind::Index | NodeKind::Paren) {
            root = source.nodes[root]
                .children
                .first()
                .copied()
                .ok_or_else(|| TypeError::unsupported(source, root))?;
        }
        if let Some(Symbol::Binding { span, .. }) = self.unit.symbol(root) {
            let stored = self.origin(right_type);
            let origin = if root == place {
                stored
            } else {
                self.binding_origin(span).join(&stored.whole())
            };
            self.set_binding_origin(*span, origin);
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
                .normalize_at(data, self.unit.file, node_index)
                .map_err(|message| TypeError::at(self.unit.file, node_index, message));
        }
        if matches!(self.catalog.types.open_data(owner), ValueType::Nominal(_)) {
            return Ok(ValueType::Nominal(Box::new(owner.clone())));
        }
        expected
            .filter(|ty| ty.nominal_parts().is_some_and(|(id, _)| id == owner))
            .cloned()
            .ok_or_else(|| TypeError::at(self.unit.file, node_index, "generic case needs its carrier application"))
    }
    pub(super) fn record(
        &mut self,
        node_index: usize,
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
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
        let order = &self.catalog.field_order[id];
        let mut origins = vec![Origin::Caller; order.len()];
        for &init in inits {
            if source.nodes[init].kind == NodeKind::Omission {
                omission = true;
                continue;
            }
            if source.nodes[init].kind != NodeKind::FieldInit {
                return Err(TypeError::unsupported(source, init));
            }
            let name = self.unit.member_name(init)?;
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
                let value = self
                    .locals
                    .get(span)
                    .cloned()
                    .ok_or_else(|| TypeError::at(source, init, "missing type of shorthand binding"))?;
                if let Some(access) = self.borrowed_binding(span) {
                    self.read_borrowed(init, &access, &value)?;
                }
                (init, value)
            } else {
                let expr = self.unit.single_child(init)?;
                (expr, self.type_expression_with_expected_type(expr, Some(&target))?)
            };
            let target = super::existential_records::select(&target, &value, owner, &mut witnesses)
                .ok_or_else(|| TypeError::at(source, site, "inconsistent package witness"))?;
            self.check_destination(site, &value, &target)?;
            let origin = match self.unit.symbol(init) {
                Some(Symbol::Binding { span, .. }) if site == init => self.binding_origin(span),
                _ => self.origin(site),
            };
            if let Some(position) = order.iter().position(|field| field == name) {
                origins[position] = origin;
            }
        }
        self.set_origin(node_index, Origin::parts(origins));
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
        let (ty, borrowed) = self.project_fields(node_index, &Access::Shared)?;
        if let Some(access) = borrowed {
            self.read_borrowed(node_index, &access, &ty)?;
        }
        Ok(ty)
    }
    /// Select a field chain as a place. The second result is the access of
    /// the borrowed storage the selected field lives in, when any selector or
    /// the root reaches it through a reference; reading that field as a
    /// value is then a read out of borrowed storage.
    pub(super) fn project_fields(
        &mut self,
        mut node_index: usize,
        access: &Access,
    ) -> Result<(ValueType, Option<Access>), TypeError> {
        let destination = node_index;
        let source = self.unit.file;
        // Postfix selectors form arbitrarily long, left-nested trees in L1.
        let mut selectors = Vec::new();
        while source.nodes[node_index].kind == NodeKind::FieldAccess {
            selectors.push(node_index);
            node_index = self.unit.single_child(node_index)?;
        }
        let (mut ty, mut borrowed) = self.place_operand(node_index)?;
        let mut origin = self.origin(node_index);
        let mut backing = self.backing(node_index);
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
                ValueType::Reference(through, inner) => {
                    // The field lives wherever the reference points.
                    borrowed = Some(borrowed.map_or(through.clone(), |outer| weaker(&outer, through)));
                    origin = origin.whole();
                    backing = origin.clone();
                    inner.as_ref()
                }
                other => other,
            };
            let field = self.unit.member_name(node_index)?;
            if matches!(receiver, ValueType::Slice(_) | ValueType::Array(_, _) | ValueType::IndexedArray(..)) && matches!(field, "len" | "address") {
                if *access != Access::Shared {
                    return Err(TypeError::at(source, node_index, "collection metadata is read-only"));
                }
                let address = field == "address";
                ty = if address { ValueType::Address } else { ValueType::integer("u64").unwrap() };
                // Metadata describes the collection; it reads no element.
                borrowed = None;
                origin = Origin::Caller;
                self.set_origin(node_index, Origin::Caller);
                self.set_backing(node_index, backing.clone());
                self.output.expressions.push(TypedExpression {
                    node: source.node_id(node_index),
                    site: Site::at(source, node_index),
                    ty: ty.clone(),
                });
                continue;
            }
            let Some((id, args)) = receiver.nominal_parts() else {
                return Err(TypeError::at(source, node_index, format!("cannot select a field of {ty}")));
            };
            let shape = case.take();
            let shape = shape.as_ref().unwrap_or(id);
            if let Some(position) = self.catalog.field_order[shape]
                .iter()
                .position(|name| name == field)
            {
                origin = origin.component(position);
            }
            ty = self
                .catalog
                .fields
                .get(shape)
                .and_then(|fields| fields.get(field))
                .map(|t| super::existential_records::open(&self.catalog.types.field_type(t, id, args), id, &opening))
                .ok_or_else(|| TypeError::at(source, node_index, format!("{} has no field {}", id.path, field)))?;
            self.set_origin(node_index, origin.clone());
            self.set_backing(node_index, backing.clone());
            opening.fields.push(field.into());
            self.output.expressions.push(TypedExpression {
                node: source.node_id(node_index),
                site: Site::at(source, node_index),
                ty: ty.clone(),
            });
        }
        Ok((ty, borrowed))
    }
}
