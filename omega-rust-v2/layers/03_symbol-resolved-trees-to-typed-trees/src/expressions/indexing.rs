use crate::expressions::body_typer::BodyTyper;
use crate::type_error::SourceTypeError;
use crate::type_relations::StoredReference;
use crate::{Access, CollectionAccess, ObligationKind, TypeError, ValueType};
use omega_toolchain::CollectionPrimitive;
use omega_typed::site_at;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum IndexUse {
    Ordinary,
    Projection(ProjectionUse),
    MovePrefix,
    MoveOperand,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ProjectionUse {
    Read,
    Shared,
    Exclusive,
}

impl BodyTyper<'_, '_> {
    pub(super) fn index(
        &mut self,
        node_index: usize,
        requested: Option<Access>,
    ) -> Result<ValueType, TypeError> {
        self.index_operand(node_index, requested, IndexUse::Ordinary).map(|(value, _)| value)
    }

    /// A move prefix selects storage for the following projection. Only the final
    /// operand can extract a value through a loan. A projection returns the
    /// element place's carrier and access instead of the selected loan value.
    pub(super) fn index_operand(
        &mut self,
        node_index: usize,
        requested: Option<Access>,
        use_site: IndexUse,
    ) -> Result<(ValueType, Option<Access>), TypeError> {
        let source = self.unit.file;
        let [base, selector] = *source.nodes[node_index].children.as_slice() else {
            return Err(TypeError::unsupported(source, node_index));
        };
        // A binding of a borrowed field names that field in place, so the
        // collection is reached with the borrowed storage's access.
        let (base_ty, borrowed) = match use_site {
            IndexUse::MovePrefix | IndexUse::MoveOperand if source.nodes[source.ungroup(base)].kind == omega_syntax::NodeKind::Index => {
                (self.index_move_place(base, IndexUse::MovePrefix)?, None)
            }
            IndexUse::Projection(projection) => self.projection_operand(base, projection)?,
            _ if requested == Some(Access::Mut) => self.projection_operand(base, ProjectionUse::Exclusive)?,
            _ => self.place_operand(base)?,
        };
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
        let backing = if reference.is_some() {
            self.origin(base)
        } else {
            self.storage_place(base).backing
        };
        let contents = if reference.is_some() { self.contents(&backing) } else { self.origin(base) };
        let backing = backing.project(super::storage_origin::Component::Element);
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
                return Ok((result, None));
            }
        };
        let copy_element = self.copy_eligible(element);
        if matches!(use_site, IndexUse::MovePrefix | IndexUse::MoveOperand)
            && requested.is_none()
            && range.is_none()
            && access.is_none()
            && matches!(storage, ValueType::Array(..))
            && !copy_element
        {
            let element = element.clone();
            return self.owned_element(node_index, base, selector, element).map(|value| (value, None));
        }
        // `move view[k]` through an exclusive element view selects the element
        // exactly as an exclusive element borrow does, then moves the non-copy
        // element out, leaving restoration debt on that view's loan. A shared
        // view and an owned array keep their own rules.
        if use_site == IndexUse::MovePrefix {
            return Err(TypeError::at(source, node_index, "nested indexed move requires an owned fixed-array place"));
        }
        let extraction = use_site == IndexUse::MoveOperand
            && requested.is_none()
            && range.is_none()
            && reference.is_some()
            && access == Some(Access::Mut)
            && matches!(storage, ValueType::Slice(_))
            && !copy_element;
        // A selector used as a place selects an element loan. A field value
        // read may continue to use the by-copy operation for a copy element.
        // A bare range still forms its ordinary shared view.
        let projected = match use_site {
            IndexUse::Projection(ProjectionUse::Exclusive) if range.is_none() => Some(Access::Mut),
            IndexUse::Projection(ProjectionUse::Shared) if range.is_none() => Some(Access::Shared),
            IndexUse::Projection(ProjectionUse::Read) if range.is_none() && !copy_element => Some(Access::Shared),
            _ => None,
        };
        let requested = if extraction { Some(Access::Mut) } else { projected.clone().or(requested) };
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
            && !copy_element
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
        let selector_carriers = signature.parameters[1..]
            .iter()
            .map(|(_, carrier)| carrier.substitute(machine, element_args))
            .collect();
        let result = signature.result.substitute(machine, element_args);
        self.record_place(
            node_index,
            base,
            reference.map(|(a, _)| a),
            Some(crate::Projection::Index(source.node_id(selector))),
        );
        self.set_origin(
            node_index,
            if !extraction && projected.is_none() && (requested.is_some() || range.is_some()) {
                backing.clone()
            } else {
                contents
            },
        );
        self.places.insert(
            node_index,
            super::storage_origin::StoragePlace {
                access: requested.clone(),
                backing,
            },
        );
        self.record_obligation(node_index, ObligationKind::CallPrecondition(machine.clone()));
        self.record_obligation(
            node_index,
            ObligationKind::Borrow {
                access: requested.unwrap_or(Access::Shared),
                place: site_at(source, base),
            },
        );
        self.output.collection_accesses.push(CollectionAccess {
            site: site_at(source, node_index),
            node: source.node_id(node_index),
            machine: machine.clone(),
            primitive,
            selector_carriers,
            element: element.clone(),
            result: result.clone(),
        });
        if extraction {
            let moved = element.clone();
            self.read_borrowed(node_index, &Access::Mut, &moved)?;
            return Ok((moved, None));
        }
        if projected.is_some() {
            return Ok((element.clone(), projected));
        }
        Ok((result, None))
    }
}
