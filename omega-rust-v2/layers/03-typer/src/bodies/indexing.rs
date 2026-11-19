use super::*;
use crate::{CollectionAccess, CollectionSelectors};

impl Body<'_, '_> {
    pub(super) fn index(
        &mut self,
        n: usize,
        requested: Option<Access>,
    ) -> Result<ValueType, TypeError> {
        let f = &self.unit.file;
        let [base, selector] = *f.nodes[n].children.as_slice() else {
            return Err(TypeError::unsupported(f, n));
        };
        let base_ty = self.expression(base)?;
        let underlying = match &base_ty {
            ValueType::AtLifetime(t, _) => t.as_ref(),
            other => other,
        };
        let (access, storage) = match underlying {
            ValueType::Reference(a, t) => (Some(a), t.as_ref()),
            t => (None, t),
        };
        let range = f.nodes[selector].kind == NodeKind::Range;
        let element = match storage {
            ValueType::Slice(t) | ValueType::Array(t, _) | ValueType::IndexedArray(t, _) => t.as_ref(),
            _ => {
                let mut args = vec![(base, base_ty.clone())];
                let token = if range { "[..]" } else { "[]" };
                if range {
                    if f.nodes[selector].children.len() != 2 {
                        return Err(TypeError::at(f, selector, "open range needs a collection with a known live extent"));
                    }
                    for &bound in &f.nodes[selector].children {
                        args.push((bound, self.expression(bound)?));
                    }
                } else {
                    args.push((selector, self.expression(selector)?));
                }
                let operands = args.iter().map(|(n, ty)| (*n, ty)).collect::<Vec<_>>();
                let result = self
                    .declared_operator(n, token, &operands)?
                    .ok_or_else(|| TypeError::at(f, n, format!("no admitted {token} declaration for {base_ty}")))?;
                if let Some(requested) = requested
                    && !matches!(&result,ValueType::Reference(access,_) if access==&requested || requested==Access::Shared && access==&Access::Mut)
                {
                    return Err(TypeError::at(f, n, "selected index result lacks the requested storage access"));
                }
                return Ok(result);
            }
        };
        let name = match (&requested, range) {
            (Some(Access::Write), _) => return Err(TypeError::at(f, n, "no write-only collection adaptation")),
            (Some(Access::Mut), false) => "element_mut",
            (Some(Access::Mut), true) => "range_mut",
            (Some(Access::Shared), false) => "element",
            (_, true) => "range",
            (None, false) => "read",
        };
        if requested == Some(Access::Mut) {
            match access {
                Some(Access::Mut) => {}
                None => self.require_writable(base)?,
                _ => return Err(TypeError::at(f, base, "collection requires mutable authority")),
            }
        } else if access == Some(&Access::Write) {
            return Err(TypeError::at(f, base, "cannot read through a write-only collection"));
        }
        let contract = crate::collections::signature(name);
        if name == "read" && self.catalog.types.multiplicity(element) != crate::Multiplicity::Copy {
            return Err(TypeError::at(f, n, "indexed read requires a copy element"));
        }
        let element_args = std::slice::from_ref(element);
        let selectors = if range {
            let node = &f.nodes[selector];
            let delimiter = node
                .leaves
                .iter()
                .find(|l| l.text.starts_with(".."))
                .ok_or_else(|| TypeError::unsupported(f, selector))?;
            let mut start = None;
            let mut end = None;
            for &bound in &node.children {
                let is_start = f.span(bound).get_start_offset() < delimiter.span.get_start_offset();
                let parameter = &contract.parameters[if is_start { 1 } else { 2 }].1;
                let actual = self.expected(bound, Some(parameter))?;
                self.destination(bound, &actual, parameter)?;
                let occurrence = Some(Site::at(f, bound));
                if is_start {
                    start = occurrence;
                } else {
                    end = occurrence;
                }
            }
            CollectionSelectors::Range {
                start,
                end,
                inclusive: delimiter.text == "..=",
            }
        } else {
            let parameter = &contract.parameters[1].1;
            let value = self.expected(selector, Some(parameter))?;
            self.destination(selector, &value, parameter)?;
            CollectionSelectors::Index(Site::at(f, selector))
        };
        let result = contract.result.substitute(&contract.machine, element_args);
        self.obligation(n, ObligationKind::CallPrecondition(contract.machine.clone()));
        self.obligation(
            n,
            ObligationKind::Borrow {
                access: requested.unwrap_or(Access::Shared),
                place: Site::at(f, base),
            },
        );
        self.output.collection_accesses.push(CollectionAccess {
            site: Site::at(f, n),
            collection: Site::at(f, base),
            machine: contract.machine.clone(),
            element: element.clone(),
            result: result.clone(),
            selectors,
        });
        Ok(result)
    }
}
