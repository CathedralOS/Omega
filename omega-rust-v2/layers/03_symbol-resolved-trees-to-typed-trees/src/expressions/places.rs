use crate::expressions::body_typer::BodyTyper;
use crate::expressions::storage_origin::Origin;
use crate::type_system::domains;
use crate::{Access, ObligationKind, Site, TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::Symbol;
use std::collections::HashSet;

pub(super) enum PlaceUse {
    Assignment,
    Borrow { site: usize, access: Access },
}

impl BodyTyper<'_, '_> {
    pub(super) fn record_place(
        &mut self,
        node: usize,
        base: usize,
        through: Option<&crate::Access>,
        projection: Option<crate::Projection>,
    ) {
        let source = self.unit.file;
        let base = source.node_id(source.ungroup(base));
        let node = source.node_id(node);
        if node != base {
            self.output.place_steps.insert(
                node,
                crate::PlaceStep {
                    base,
                    through: through.cloned(),
                    projection,
                },
            );
        }
    }

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
                match expected {
                    Some(ty) => ty.carrier().clone(),
                    None => {
                        let (id, written) = self.unit.domain_reference(path, None, &self.catalog.types)?;
                        written.unwrap_or_else(|| self.catalog.types.domains.carrier(id).carrier().clone())
                    }
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
                .expanded_domain_at(&domain, &carrier, false, self.unit, entry)
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
                .expanded_domain_at(atom, &carrier, true, self.unit, node_index)
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
        place: usize,
        usage: PlaceUse,
    ) -> Result<(usize, ValueType), TypeError> {
        let access = match &usage {
            PlaceUse::Assignment => &Access::Write,
            PlaceUse::Borrow { access, .. } => access,
        };
        let source = self.unit.file;
        let place = source.ungroup(place);
        let ty = match source.nodes[place].kind {
            NodeKind::FieldAccess => self.project_fields(place, access)?.0,
            NodeKind::Index => {
                let ty = match usage {
                    PlaceUse::Assignment => {
                        let view = self.index(place, Some(Access::Mut))?;
                        let ValueType::Reference(Access::Mut | Access::Write, inner) = view else {
                            return Err(TypeError::at(source, place, "index does not return writable storage"));
                        };
                        *inner
                    }
                    PlaceUse::Borrow { access, .. } => self.index(place, Some(access))?,
                };
                return Ok((place, ty));
            }
            _ => self.place_operand(place)?.0,
        };
        self.require_place_access(place, access)?;
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
    pub(super) fn require_place_access(
        &self,
        place: usize,
        requested: &Access,
    ) -> Result<(), TypeError> {
        match (self.storage_place(place).access, requested) {
            (Access::Shared, Access::Write | Access::Mut) => Err(TypeError::at(self.unit.file, place, "place requires writable authority")),
            (Access::Write, Access::Shared | Access::Mut) => Err(TypeError::at(self.unit.file, place, "cannot read through a write-only reference")),
            _ => Ok(()),
        }
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
        self.store_origin(place, self.origin(right_type));
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
                .data_application(node_index, owner, &self.catalog.types)?;
            return self
                .catalog
                .types
                .normalize_at(data, self.unit, node_index)
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
        } else if let Some(ty @ ValueType::Evidence(slot)) = expected
            && source.nodes[*head].children.is_empty()
            && matches!(self.unit.symbol(*head),Some(Symbol::Declaration(id)) if id==&slot.declaration)
        {
            ty.clone()
        } else {
            self.unit
                .initialized_annotation(*head, &self.catalog.types, expected)?
        };
        let (owner, args) = ty
            .record_parts()
            .ok_or_else(|| TypeError::at(source, *head, "record literal requires a record type"))?;
        self.unit.require_selection_authority(*head, owner)?;
        if self.catalog.refinements.contains_key(owner) {
            return Err(TypeError::at(source, *head, "a transparent refinement requires a checked base conformance"));
        }
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
        let mut witnesses = vec![None; self.catalog.types.witness_count(owner)];
        let mut lifetimes = crate::type_system::local_lifetimes::FieldLifetimes::default();
        let infer_regions = args
            .iter()
            .flat_map(ValueType::components)
            .any(|arg| matches!(arg, ValueType::InferredLifetime { .. }));
        let mut omission = false;
        let mut origins = vec![Origin::Caller; fields.ordered.len()];
        for &init in inits {
            if source.nodes[init].kind == NodeKind::Omission {
                omission = true;
                continue;
            }
            if source.nodes[init].kind != NodeKind::FieldInit {
                return Err(TypeError::unsupported(source, init));
            }
            let name = self.unit.member_name(init)?;
            let (position, declared) = fields
                .get(name)
                .ok_or_else(|| TypeError::at(source, init, format!("{} has no field {name}", id.path)))?;
            self.unit
                .require_selection_authority(init, &declared.declaration)?;
            let mut target = self
                .catalog
                .types
                .apply_type_arguments(&declared.ty, owner, args);
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
                if let Some(access) = self.borrowed_binding(init, span) {
                    self.read_borrowed(init, &access, &value)?;
                }
                (init, value)
            } else {
                let expr = self.unit.single_child(init)?;
                let package_hint;
                let hint = if witnesses.is_empty() {
                    Some(&target)
                } else {
                    package_hint = self.package_field_hint(expr, &target, owner, &mut witnesses)?;
                    package_hint.as_ref()
                };
                (expr, self.type_expression_with_expected_type(expr, hint)?)
            };
            if !witnesses.is_empty() {
                super::existential_records::infer_witnesses(&target, &value, owner, &mut witnesses)
                    .ok_or_else(|| TypeError::at(source, site, "inconsistent package witness"))?;
                target = super::existential_records::replace_witnesses(&target, owner, &|i| witnesses[i].clone());
            }
            if infer_regions {
                lifetimes.observe(&target, &value);
            }
            self.check_destination(site, &value, &target)?;
            let origin = match self.unit.symbol(init) {
                Some(Symbol::Binding { span, .. }) if site == init => self.binding_origin(span),
                _ => self.origin(site),
            };
            origins[position] = if origin.is_local() && self.carried_loans(&target, origin.clone()).is_none() {
                Origin::Caller
            } else {
                origin
            };
        }
        let mut origin = Origin::parts(origins);
        if let Some(case) = case {
            origin = Origin::Case(Box::new(case.clone()), Box::new(origin));
        }
        self.set_origin(node_index, origin);

        let ty = lifetimes.finish(ty);
        let (owner, args) = ty.record_parts().unwrap();
        for field in fields.ordered.iter() {
            let name = field.name();
            if !seen.contains(name) {
                if !omission || owner.kind == NodeKind::Trait {
                    return Err(TypeError::at(source, node_index, format!("missing field {name}")));
                }
                let mut target = self.catalog.types.apply_type_arguments(&field.ty, owner, args);
                if !witnesses.is_empty() {
                    target = super::existential_records::replace_witnesses(&target, owner, &|i| witnesses[i].clone());
                }
                self.record_obligation(
                    node_index,
                    ObligationKind::ZeroField {
                        field: field.declaration.clone(),
                        ty: target,
                    },
                );
            }
        }
        if self.catalog.predicates.contains(owner) {
            self.record_obligation(node_index, ObligationKind::EstablishData(owner.clone()));
        }
        if !witnesses.is_empty() {
            let witnesses = witnesses
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| TypeError::at(source, node_index, "cannot infer every package witness"))?;
            if witnesses
                .iter()
                .any(|ty| !self.catalog.package_representation(ty, None))
            {
                return Err(TypeError::at(source, node_index, "package witness must be sized"));
            }
            self.output.packages.push(crate::PackageConstruction {
                site: Site::at(source, node_index),
                carrier: ty.clone(),
                witnesses,
            });
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
        let mut place = self.storage_place(node_index);
        let mut base = node_index;
        let mut case = self
            .fact_subject(node_index)
            .and_then(|s| self.cases.get(&s))
            .cloned();
        for node_index in selectors.into_iter().rev() {
            let reference = ty.reference_parts();
            let through = reference.map(|(access, _)| access.clone());
            let receiver = match reference {
                Some((Access::Write, _)) if *access != Access::Write || node_index != destination => {
                    return Err(TypeError::at(source, node_index, "cannot read through a write-only reference"));
                }
                Some((through, inner)) => {
                    // The field lives wherever the reference points.
                    place.access = through
                        .through(borrowed.as_ref())
                        .map_err(|m| TypeError::at(source, node_index, m))?;
                    borrowed = Some(place.access.clone());
                    origin = origin.whole();
                    place.backing = origin.clone();
                    inner
                }
                None => &ty,
            };
            let field = self.unit.member_name(node_index)?;
            if matches!(receiver, ValueType::Slice(_) | ValueType::Array(_, _) | ValueType::IndexedArray(..)) && matches!(field, "len" | "address") {
                if *access != Access::Shared {
                    return Err(TypeError::at(source, node_index, "collection metadata is read-only"));
                }
                let (operation, metadata_type) = if field == "address" {
                    (".address", ValueType::Address)
                } else {
                    (".len", ValueType::integer("u64").unwrap())
                };
                self.record_operator(node_index, operation, vec![ty], &metadata_type);
                ty = metadata_type;
                // Metadata describes the collection; it reads no element.
                borrowed = None;
                origin = Origin::Caller;
                place.access = Access::Shared;
            } else {
                let Some((id, args)) = receiver.record_parts() else {
                    return Err(TypeError::at(source, node_index, format!("cannot select a field of {ty}")));
                };
                self.unit.require_selection_authority(node_index, id)?;
                let shape = case.take();
                let shape = shape.as_ref().unwrap_or(id);
                let (position, declared) = self
                    .catalog
                    .fields
                    .get(shape)
                    .and_then(|fields| fields.get(field))
                    .ok_or_else(|| TypeError::at(source, node_index, format!("{} has no field {}", id.path, field)))?;
                self.unit
                    .require_selection_authority(node_index, &declared.declaration)?;
                origin = origin.component(position);
                ty = self
                    .catalog
                    .types
                    .opened_field(&declared.ty, id, args, || self.opening(base));
                self.record_place(
                    node_index,
                    base,
                    through.as_ref(),
                    Some(crate::Projection::Field(Box::new(declared.declaration.clone()))),
                );
            }
            self.set_origin(node_index, origin.clone());
            base = node_index;
            self.places.insert(node_index, place.clone());
            self.record_expression(node_index, ty.clone());
        }
        Ok((ty, borrowed))
    }
}
