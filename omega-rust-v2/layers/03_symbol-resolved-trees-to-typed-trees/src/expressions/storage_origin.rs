//! Whether a value can expose this invocation's own storage is a property of
//! the value, not of the expression that spells it (lifetimes.md: "What
//! qualifies is the backing a view denotes, not the form of the returning
//! expression"). The typer records that property on every expression it types
//! and on every binding a pattern introduces, so one check at the return
//! decides `let v = @x; v`, `match @x { r -> r, }`, `(@x, 0u8).0`, `move v`,
//! `@x as @u8` and record or array elements alike.
//!
//! A value's origin says whether it may hold a loan of invocation-local
//! storage. Aggregates keep one origin per component, so selecting a tuple
//! component or a record field keeps only that component's loans. Anything
//! reached through a reference keeps the reference's origin as a whole: the
//! typer does not know which loans the referent's contents carry, so it
//! assumes they share the reference's backing.
//!
//! A place's backing says whose storage the place denotes. A local binding or
//! a temporary is this invocation's storage. A place reached through a
//! reference lives wherever that reference points. A binding that a record or
//! tuple pattern introduces from a borrowed subject names the subject's field
//! in place (patterns.md: "A binding of a borrowed field retains the source's
//! borrow and backing lifetime rather than copying its carrier"), so borrowing
//! it borrows that field.
//!
//! Call results carry no origin here. The `ReturnBorrow` obligation hands a
//! returned call result to the lifetime checker with its selected signature.

use crate::expressions::body_typer::BodyTyper;
use crate::{Access, TypeError, ValueType};
use omega_lexer::SourceSpan;
use omega_parser::NodeKind;
use omega_resolver::Symbol;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(super) enum Origin {
    /// Payload fields are numbered within their active sum case.
    Case(Box<omega_resolver::DeclarationId>, Box<Origin>),
    /// Holds no loan of invocation-local storage.
    #[default]
    Caller,
    /// May hold a loan of invocation-local storage.
    Local,
    /// A tuple or record whose components have different origins, in tuple
    /// position or declared field order. At least one component is not `Caller`.
    Parts(Vec<Origin>),
}

impl Origin {
    pub(super) fn parts(parts: Vec<Origin>) -> Self {
        if parts.iter().all(|p| *p == Self::Caller) {
            Self::Caller
        } else if parts.iter().all(|p| *p == Self::Local) {
            Self::Local
        } else {
            Self::Parts(parts)
        }
    }
    pub(super) fn is_local(&self) -> bool {
        match self {
            Self::Case(_, payload) => payload.is_local(),
            Self::Caller => false,
            Self::Local => true,
            Self::Parts(parts) => parts.iter().any(Self::is_local),
        }
    }
    /// The origin of the whole value, for arrays and for contents reached
    /// through a reference, where components are not tracked separately.
    pub(super) fn whole(&self) -> Self {
        if self.is_local() { Self::Local } else { Self::Caller }
    }
    pub(super) fn component(
        &self,
        index: usize,
    ) -> Self {
        match self {
            Self::Case(_, payload) => payload.component(index),
            Self::Parts(parts) => parts.get(index).cloned().unwrap_or(Self::Local),
            other => other.clone(),
        }
    }
    pub(super) fn join(
        &self,
        other: &Self,
    ) -> Self {
        match (self, other) {
            (Self::Case(a, x), Self::Case(b, y)) if a == b => Self::Case(a.clone(), Box::new(x.join(y))),
            (Self::Case(..), _) | (_, Self::Case(..)) => self.whole().join(&other.whole()),
            (Self::Caller, x) | (x, Self::Caller) => x.clone(),
            (Self::Parts(a), Self::Parts(b)) if a.len() == b.len() => Self::parts(a.iter().zip(b).map(|(a, b)| a.join(b)).collect()),
            _ => Self::Local,
        }
    }
}

/// The access and backing storage of a selected place or a pattern alias.
#[derive(Clone, Debug)]
pub(super) struct StoragePlace {
    /// The weakest access along the references that reach the field.
    pub(super) access: Access,
    /// Whose storage the field lives in: the borrowed subject's origin.
    pub(super) backing: Origin,
}

/// What a pattern position receives: the origin of the value it matches and,
/// when that value lives behind a reference, the borrowed place it names.
/// A tuple of places, as in `match (command.id, command.op)`, keeps one
/// source per component.
#[derive(Clone, Debug, Default)]
pub(super) struct PatternSource {
    pub(super) value: Origin,
    pub(super) place: Option<StoragePlace>,
    pub(super) parts: Vec<PatternSource>,
}

impl PatternSource {
    pub(super) fn of(value: Origin) -> Self {
        Self {
            value,
            place: None,
            parts: Vec::new(),
        }
    }
    /// The source of one component of a subject of type `subject`. Fields of
    /// a referenced value live in the reference's backing.
    pub(super) fn component(
        &self,
        subject: &crate::ValueType,
        index: usize,
    ) -> Result<Self, &'static str> {
        if let Some(part) = self.parts.get(index) {
            return Ok(part.clone());
        }
        Ok(match subject.reference_parts() {
            Some((access, _)) => Self {
                value: self.value.whole(),
                place: Some(StoragePlace {
                    access: access.through(self.place.as_ref().map(|p| &p.access))?,
                    backing: self.value.whole(),
                }),
                parts: Vec::new(),
            },
            _ => Self {
                value: self.value.component(index),
                place: self.place.clone(),
                parts: Vec::new(),
            },
        })
    }
}

impl BodyTyper<'_, '_> {
    /// Return the origin of the loans in a result. Lifetimes on a data
    /// application are arguments; the stored fields determine which are carried.
    pub(super) fn carried_loans(
        &self,
        ty: &ValueType,
        origin: Origin,
    ) -> Option<Origin> {
        use ValueType::*;
        enum Work {
            Value(ValueType, Origin),
            Leave,
        }
        let mut work = vec![Work::Value(ty.clone(), origin)];
        let mut active: Vec<ValueType> = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut carried = None;
        while let Some(item) = work.pop() {
            let Work::Value(ty, origin) = item else {
                active.pop();
                continue;
            };
            match &ty {
                Reference(..) | AtLifetime(..) | Parameter { .. } | Witness { .. } => {
                    if origin.is_local() {
                        return Some(Origin::Local);
                    }
                    carried = Some(Origin::Caller);
                }
                Tuple(parts) => work.extend(
                    parts
                        .iter()
                        .enumerate()
                        .map(|(i, t)| Work::Value(t.clone(), origin.component(i))),
                ),
                Array(_, 0) => {}
                Slice(t) | Array(t, _) | IndexedArray(t, _) | Qualified(t, _) => work.push(Work::Value(*t.clone(), origin)),
                Nominal(_) | Applied(..) => {
                    let (owner, args) = ty.nominal_parts().unwrap();
                    if let Some(enclosing) = active
                        .iter()
                        .rev()
                        .find(|t| t.nominal_parts().is_some_and(|(id, _)| id == owner))
                        && !enclosing.components().skip(1).any(|t| t == &ty)
                    {
                        // An unresolved recursive layout still needs a verifier judgment.
                        carried = Some(Origin::Caller);
                        continue;
                    }
                    if !visited.insert((ty.clone(), origin.clone())) {
                        continue;
                    }
                    work.push(Work::Leave);
                    let active_case = match &origin {
                        Origin::Case(case, _) => Some(case.as_ref()),
                        _ => None,
                    };
                    work.extend(
                        self.catalog
                            .stored_fields(owner, active_case)
                            .map(|(i, field)| Work::Value(self.catalog.types.apply_type_arguments(&field.ty, owner, args), origin.component(i))),
                    );
                    active.push(ty);
                }
                _ => {}
            }
        }
        carried
    }

    /// Type an operand that names a place without reading it. A binding of a
    /// field of borrowed storage names that field in place, and so does a
    /// field selection, so a borrow, assignment, field selection or index
    /// through either reads nothing yet. The access it returns is that of the
    /// borrowed storage the place lives in, if any.
    pub(super) fn place_operand(
        &mut self,
        node_index: usize,
    ) -> Result<(ValueType, Option<Access>), TypeError> {
        let source = self.unit.file;
        // A grouped receiver, as in `(view.inner).value`, is still a place.
        if source.nodes[node_index].kind == NodeKind::Paren {
            let (ty, borrowed) = self.place_operand(self.unit.single_child(node_index)?)?;
            self.set_origin(node_index, self.origin(self.unit.single_child(node_index)?));
            self.record_expression(node_index, ty.clone());
            return Ok((ty, borrowed));
        }
        if source.nodes[node_index].kind == NodeKind::FieldAccess {
            return self.project_fields(node_index, &Access::Shared);
        }
        if source.nodes[node_index].kind == NodeKind::PathExpression
            && let Some(Symbol::Binding { span, .. }) = self.unit.symbol(node_index)
            && let Some(access) = self.borrowed_binding(node_index, span)
        {
            let ty = self
                .locals
                .get(span)
                .cloned()
                .ok_or_else(|| TypeError::at(source, node_index, "missing type of resolved binding"))?;
            self.set_origin(node_index, self.binding_origin(span));
            self.record_expression(node_index, ty.clone());
            return Ok((ty, Some(access)));
        }
        Ok((self.type_expression(node_index)?, None))
    }
    /// Type the subject a pattern decomposes. A dispatch or destructuring
    /// inspects its subject in place: matching `command.op` with
    /// `command: @Command` neither copies nor moves the field out of the
    /// borrowed parent (patterns.md), and the bindings name its fields. A
    /// tuple of such subjects keeps each component's place.
    pub(super) fn pattern_subject(
        &mut self,
        node_index: usize,
    ) -> Result<(ValueType, PatternSource), TypeError> {
        let source = self.unit.file;
        if source.nodes[node_index].kind == NodeKind::Paren {
            let (ty, part) = self.pattern_subject(self.unit.single_child(node_index)?)?;
            self.set_origin(node_index, part.value.clone());
            self.record_expression(node_index, ty.clone());
            return Ok((ty, part));
        }
        if source.nodes[node_index].kind == NodeKind::Tuple {
            let mut types = Vec::new();
            let mut parts = Vec::new();
            for &child in &source.nodes[node_index].children {
                let (ty, part) = self.pattern_subject(child)?;
                types.push(ty);
                parts.push(part);
            }
            let ty = ValueType::Tuple(types);
            let value = Origin::parts(parts.iter().map(|p| p.value.clone()).collect());
            self.set_origin(node_index, value.clone());
            self.record_expression(node_index, ty.clone());
            return Ok((ty, PatternSource { value, place: None, parts }));
        }
        let (ty, borrowed) = self.place_operand(node_index)?;
        let place = borrowed.map(|access| StoragePlace {
            access,
            backing: self.storage_place(node_index).backing,
        });
        Ok((
            ty,
            PatternSource {
                value: self.origin(node_index),
                place,
                parts: Vec::new(),
            },
        ))
    }
    /// The access of the borrowed storage a binding names in place, if any.
    pub(super) fn borrowed_binding(
        &mut self,
        node: usize,
        span: &SourceSpan,
    ) -> Option<Access> {
        let (binder, place) = self.borrowed_places.get(span)?;
        let access = place.access.clone();
        self.record_place(node, *binder, None, None);
        Some(access)
    }
    /// The origin of a typed expression.
    pub(super) fn origin(
        &self,
        node_index: usize,
    ) -> Origin {
        self.origins.get(&node_index).cloned().unwrap_or_default()
    }
    pub(super) fn set_origin(
        &mut self,
        node_index: usize,
        origin: Origin,
    ) {
        if origin == Origin::Caller {
            self.origins.remove(&node_index);
        } else {
            self.origins.insert(node_index, origin);
        }
    }
    pub(super) fn binding_origin(
        &self,
        span: &SourceSpan,
    ) -> Origin {
        self.binding_origins.get(span).cloned().unwrap_or_default()
    }
    pub(super) fn set_binding_origin(
        &mut self,
        span: SourceSpan,
        origin: Origin,
    ) {
        if origin == Origin::Caller {
            self.binding_origins.remove(&span);
        } else {
            self.binding_origins.insert(span, origin);
        }
    }
    /// Record what a fresh binder receives from its pattern position.
    pub(super) fn bind_source(
        &mut self,
        span: SourceSpan,
        binder: usize,
        ty: ValueType,
        source: &PatternSource,
    ) {
        self.locals.insert(span, ty);
        self.set_binding_origin(span, source.value.clone());
        match &source.place {
            Some(place) => self.borrowed_places.insert(span, (binder, place.clone())),
            None => self.borrowed_places.remove(&span),
        };
    }
    /// Assign through the selected storage, including pattern names for that storage.
    pub(super) fn store_origin(
        &mut self,
        place: usize,
        stored: Origin,
    ) {
        let file = self.unit.file;
        let (root, path) = self.output.place_path(file.node_id(place));
        let binding = |node: omega_resolver::TreeNodeId| match self.unit.symbol(node.node) {
            Some(Symbol::Binding { binder_node, .. }) => file.node_id(*binder_node),
            _ => node,
        };
        // A direct binding assignment replaces the binding. An alias captured
        // from its old referent continues to denote that evaluated place.
        let mut updates = Vec::new();
        if !path.is_empty() {
            for (&span, (binder, _)) in &self.borrowed_places {
                let (base, alias) = self.output.place_path(file.node_id(*binder));
                if binding(base) != binding(root)
                    || !path
                        .iter()
                        .zip(&alias)
                        .all(|(a, b)| a.projection == b.projection)
                {
                    continue;
                }
                // Separate evaluations can observe different referents of a mutable binding.
                let value = if root == base && path.len() == alias.len() {
                    stored.clone()
                } else {
                    self.binding_origin(&span).join(&stored.whole())
                };
                updates.push((span, value));
            }
        }
        if let Some(Symbol::Binding { span, .. }) = self.unit.symbol(root.node) {
            let origin = if path.is_empty() {
                stored
            } else {
                self.binding_origin(span).join(&stored.whole())
            };
            updates.push((*span, origin));
        }
        for (span, value) in updates {
            self.set_binding_origin(span, value);
        }
    }
    pub(super) fn storage_place(
        &self,
        node: usize,
    ) -> StoragePlace {
        let source = self.unit.file;
        let node = source.ungroup(node);
        if let Some(place) = self.places.get(&node) {
            return place.clone();
        }
        if let Some(Symbol::Binding { span, .. }) = self.unit.symbol(node) {
            return self
                .borrowed_places
                .get(span)
                .map(|(_, place)| place.clone())
                .unwrap_or_else(|| StoragePlace {
                    access: if self.writable.contains(span) { Access::Mut } else { Access::Shared },
                    backing: Origin::Local,
                });
        }
        let backing = match source.nodes[node].kind {
            NodeKind::PathExpression | NodeKind::Index => Origin::Caller,
            _ => Origin::Local,
        };
        StoragePlace {
            access: Access::Shared,
            backing,
        }
    }
}
