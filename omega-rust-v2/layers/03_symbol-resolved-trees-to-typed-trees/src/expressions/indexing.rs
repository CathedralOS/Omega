use crate::expressions::body_typer::BodyTyper;
use crate::toolchain_declarations::collection_contracts::CollectionPrimitive;
use crate::{Access, CollectionAccess, CollectionSelectors, ObligationKind, Site, TypeError, ValueType};
use omega_parser::NodeKind;

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
        let underlying = match &base_ty {
            ValueType::AtLifetime(t, _) => t.as_ref(),
            other => other,
        };
        let (access, storage) = match underlying {
            ValueType::Reference(a, t) => (Some(a), t.as_ref()),
            t => (borrowed.as_ref(), t),
        };
        let range = source.nodes[selector].kind == NodeKind::Range;
        // An element of a referenced collection lives wherever the reference
        // points; an element of an owned collection lives in that collection.
        let contents = self.origin(base).whole();
        let backing = if access.is_some() { contents.clone() } else { self.backing(base) };
        let element = match storage {
            ValueType::Slice(t) | ValueType::Array(t, _) | ValueType::IndexedArray(t, _) => t.as_ref(),
            _ => {
                let mut args = vec![(base, base_ty.clone())];
                let token = if range { "[..]" } else { "[]" };
                if range {
                    if source.nodes[selector].children.len() != 2 {
                        return Err(TypeError::at(source, selector, "open range needs a collection with a known live extent"));
                    }
                    for &bound in &source.nodes[selector].children {
                        args.push((bound, self.type_expression(bound)?));
                    }
                } else {
                    args.push((selector, self.type_expression(selector)?));
                }
                let operands = args
                    .iter()
                    .map(|(node_index, ty)| (*node_index, ty))
                    .collect::<Vec<_>>();
                let result = self
                    .declared_operator(node_index, token, &operands)?
                    .ok_or_else(|| TypeError::at(source, node_index, format!("no admitted {token} declaration for {base_ty}")))?;
                if let Some(requested) = requested
                    && !matches!(&result,ValueType::Reference(access,_) if access==&requested || requested==Access::Shared && access==&Access::Mut)
                {
                    return Err(TypeError::at(source, node_index, "selected index result lacks the requested storage access"));
                }
                return Ok(result);
            }
        };
        let primitive = match (&requested, range) {
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
                None => self.require_writable(base)?,
                _ => return Err(TypeError::at(source, base, "collection requires mutable authority")),
            }
        } else if access == Some(&Access::Write) {
            return Err(TypeError::at(source, base, "cannot read through a write-only collection"));
        }
        let contract = primitive.signature();
        if primitive.requires_copy() && self.catalog.types.multiplicity(element) != crate::Multiplicity::Copy {
            return Err(TypeError::at(source, node_index, "indexed read requires a copy element"));
        }
        let element_args = std::slice::from_ref(element);
        let selectors = if range {
            let node = &source.nodes[selector];
            let delimiter = self
                .unit
                .operator(selector)
                .ok_or_else(|| TypeError::unsupported(source, selector))?;
            let mut start = None;
            let mut end = None;
            for &bound in &node.children {
                let is_start = source.span(bound).get_start_offset() < delimiter.span.get_start_offset();
                let parameter = &contract.parameters[if is_start { 1 } else { 2 }].1;
                let actual = self.type_expression_with_expected_type(bound, Some(parameter))?;
                self.check_destination(bound, &actual, parameter)?;
                let occurrence = Some(Site::at(source, bound));
                if is_start {
                    start = occurrence;
                } else {
                    end = occurrence;
                }
            }
            CollectionSelectors::Range {
                start,
                end,
                inclusive: delimiter.token == "..=",
            }
        } else {
            let parameter = &contract.parameters[1].1;
            let value = self.type_expression_with_expected_type(selector, Some(parameter))?;
            self.check_destination(selector, &value, parameter)?;
            CollectionSelectors::Index(Site::at(source, selector))
        };
        let result = contract.result.substitute(&contract.machine, element_args);
        self.set_origin(node_index, if requested.is_some() || range { backing.clone() } else { contents });
        self.set_backing(node_index, backing);
        self.record_obligation(node_index, ObligationKind::CallPrecondition(contract.machine.clone()));
        self.record_obligation(
            node_index,
            ObligationKind::Borrow {
                access: requested.unwrap_or(Access::Shared),
                place: Site::at(source, base),
            },
        );
        self.output.collection_accesses.push(CollectionAccess {
            site: Site::at(source, node_index),
            collection: Site::at(source, base),
            machine: contract.machine.clone(),
            element: element.clone(),
            result: result.clone(),
            selectors,
        });
        Ok(result)
    }
}
