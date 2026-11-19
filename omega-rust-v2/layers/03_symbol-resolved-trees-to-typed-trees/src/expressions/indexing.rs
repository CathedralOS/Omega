use crate::expressions::body_typer::BodyTyper;
use crate::{Access, CollectionAccess, ObligationKind, Site, TypeError, ValueType};
use omega_toolchain::CollectionPrimitive;

impl BodyTyper<'_, '_> {
    pub(super) fn index(
        &mut self,
        node_index: usize,
        requested: Option<Access>,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        let [base, selector] = *source.nodes[node_index].children.as_slice() else {
            return Err(TypeError::unsupported(source, node_index));
        };
        // A binding of a borrowed field names that field in place, so the
        // collection is reached with the borrowed storage's access.
        let (base_ty, borrowed) = self.place_operand(base)?;
        let reference = base_ty.reference_parts();
        let (access, storage) = match reference {
            Some((access, inner)) => (
                Some(
                    access
                        .through(borrowed.as_ref())
                        .map_err(|m| TypeError::at(source, base, m))?,
                ),
                inner,
            ),
            None => (borrowed, &base_ty),
        };
        let range = source.range_bounds(selector);
        // An element of a referenced collection lives wherever the reference
        // points; an element of an owned collection lives in that collection.
        let contents = self.origin(base).whole();
        let backing = if reference.is_some() {
            contents.clone()
        } else {
            self.storage_place(base).backing
        };
        let element = match storage {
            ValueType::Slice(t) | ValueType::Array(t, _) | ValueType::IndexedArray(t, _) => t.as_ref(),
            _ => {
                let mut actual = base_ty.clone();
                let carrier = match &mut actual {
                    ValueType::AtLifetime(inner, _) => inner.as_mut(),
                    other => other,
                };
                if let (ValueType::Reference(declared, _), Some(through)) = (carrier, &access) {
                    *declared = through.clone();
                }
                let mut receiver = self.call_operand(base, actual);
                if reference.is_none() {
                    receiver.read_from = access;
                }
                let mut args = vec![receiver];
                let token = if range.is_some() { "[..]" } else { "[]" };
                if let Some(bounds) = range {
                    let (Some(start), Some(end)) = (bounds.start, bounds.end) else {
                        return Err(TypeError::at(source, selector, "open range needs a collection with a known live extent"));
                    };
                    for bound in [start, end] {
                        let value = self.type_expression(bound)?;
                        args.push(self.call_operand(bound, value));
                    }
                } else {
                    let value = self.type_expression(selector)?;
                    args.push(self.call_operand(selector, value));
                }
                let result = self
                    .declared_operator(node_index, token, &args)?
                    .ok_or_else(|| TypeError::at(source, node_index, format!("no admitted {token} declaration for {base_ty}")))?;
                if let Some(requested) = requested
                    && !matches!(&result,ValueType::Reference(access,_) if access==&requested || requested==Access::Shared && access==&Access::Mut)
                {
                    return Err(TypeError::at(source, node_index, "selected index result lacks the requested storage access"));
                }
                return Ok(result);
            }
        };
        let primitive = match (&requested, range.is_some()) {
            (Some(Access::Write), _) => return Err(TypeError::at(source, node_index, "no write-only collection adaptation")),
            (Some(Access::Mut), false) => CollectionPrimitive::ElementMut,
            (Some(Access::Mut), true) => CollectionPrimitive::RangeMut,
            (Some(Access::Shared), false) => CollectionPrimitive::Element,
            (_, true) => CollectionPrimitive::Range,
            (None, false) => CollectionPrimitive::Read,
        };
        if requested == Some(Access::Mut) {
            match access {
                Some(Access::Mut) => {}
                None => self.require_place_access(base, &Access::Mut)?,
                _ => return Err(TypeError::at(source, base, "collection requires mutable authority")),
            }
        } else if access == Some(Access::Write) {
            return Err(TypeError::at(source, base, "cannot read through a write-only collection"));
        }
        let machine = omega_toolchain::core_interface().collection(primitive);
        let signature = self.catalog.signatures.get(machine).unwrap_or_else(|| {
            crate::standard_library()
                .signatures
                .iter()
                .find(|s| &s.machine == machine)
                .expect("sealed collection signature")
        });
        if signature
            .multiplicity_bounds
            .contains(&(0, crate::Multiplicity::Copy))
            && self.catalog.types.multiplicity(element) != crate::Multiplicity::Copy
        {
            return Err(TypeError::at(source, node_index, "indexed read requires a copy element"));
        }
        let element_args = std::slice::from_ref(element);
        let bounds = range.map_or([Some(selector), None], |range| [range.start, range.end]);
        for (bound, (_, parameter)) in bounds.into_iter().zip(&signature.parameters[1..]) {
            if let Some(bound) = bound {
                let actual = self.type_expression_with_expected_type(bound, Some(parameter))?;
                self.check_destination(bound, &actual, parameter)?;
            }
        }
        let result = signature.result.substitute(machine, element_args);
        self.record_place(
            node_index,
            base,
            reference.map(|(a, _)| a),
            Some(crate::Projection::Index(source.node_id(selector))),
        );
        self.set_origin(node_index, if requested.is_some() || range.is_some() { backing.clone() } else { contents });
        self.places.insert(
            node_index,
            super::storage_origin::StoragePlace {
                access: requested.clone().unwrap_or(Access::Shared),
                backing,
            },
        );
        self.record_obligation(node_index, ObligationKind::CallPrecondition(machine.clone()));
        self.record_obligation(
            node_index,
            ObligationKind::Borrow {
                access: requested.unwrap_or(Access::Shared),
                place: Site::at(source, base),
            },
        );
        self.output.collection_accesses.push(CollectionAccess {
            site: Site::at(source, node_index),
            node: source.node_id(node_index),
            machine: machine.clone(),
            element: element.clone(),
            result: result.clone(),
        });
        Ok(result)
    }
}
