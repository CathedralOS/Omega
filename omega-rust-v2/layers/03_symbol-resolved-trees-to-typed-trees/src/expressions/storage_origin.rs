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
//! component or a record field keeps only that component's loans. A borrow of
//! a known place keeps its root and field path. Reading the reference consults the current contents of that place, including writes
//! made through other aliases.
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
use crate::type_error::SourceTypeError;
use crate::type_relations::StoredReference;
use crate::{Access, TypeError, ValueType};
use omega_resolved::Symbol;
use omega_syntax::NodeKind;
use omega_tokens::SourceSpan;

/// A formal's incoming referent lives outside the invocation. The variable
/// containing that reference is a distinct place inside the invocation.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum StorageRoot {
    Binding(SourceSpan),
    Incoming(SourceSpan, Vec<InputProjection>),
}

/// A caller-supplied aggregate can contain a reference before this body has
/// assigned any of its fields. Copies of that field denote the same referent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum InputProjection {
    Field(usize),
    Referent,
}

impl StorageRoot {
    fn initial_contents(&self) -> Origin {
        match self {
            Self::Binding(_) => Origin::Caller,
            Self::Incoming(span, path) => {
                let mut contents = path.clone();
                contents.push(InputProjection::Referent);
                Origin::reference(Self::Incoming(*span, contents))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Component {
    Field { index: usize, count: usize },
    // A dynamic index may select any element of the collection.
    Element,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct LoanTarget {
    root: StorageRoot,
    path: Vec<Component>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(super) enum Origin {
    /// Payload fields are numbered within their active sum case.
    Case(Box<omega_resolved::DeclarationId>, Box<Origin>),
    /// Evaluated targets of a reference. A join can also contain a reference
    /// with unknown identity; its lifetime still participates in return checks.
    Reference { targets: Vec<LoanTarget>, unknown_local: Option<bool> },
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
    pub(super) fn reference(root: StorageRoot) -> Self {
        Self::Reference {
            targets: vec![LoanTarget { root, path: Vec::new() }],
            unknown_local: None,
        }
    }
    pub(super) fn project(
        &self,
        component: Component,
    ) -> Self {
        let mut projected = self.clone();
        if let Self::Reference { targets, .. } = &mut projected {
            for target in targets {
                target.path.push(component);
            }
        }
        projected
    }
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
            Self::Reference { targets, unknown_local } => {
                *unknown_local == Some(true)
                    || targets
                        .iter()
                        .any(|t| matches!(t.root, StorageRoot::Binding(_)))
            }
            Self::Caller => false,
            Self::Local => true,
            Self::Parts(parts) => parts.iter().any(Self::is_local),
        }
    }
    /// Summarize the lifetime when the selected component is unknown.
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
            Self::Reference { targets, unknown_local } => {
                // Fields of an untouched input aggregate still denote the original
                // caller values, even when the aggregate has since been copied.
                let mut targets = targets.clone();
                for target in &mut targets {
                    if target.path.is_empty()
                        && let StorageRoot::Incoming(_, path) = &mut target.root
                    {
                        path.push(InputProjection::Field(index));
                    }
                }
                Self::Reference {
                    targets,
                    unknown_local: *unknown_local,
                }
            }
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
            (Self::Reference { .. }, Self::Parts(parts)) => Self::parts(
                parts
                    .iter()
                    .enumerate()
                    .map(|(i, part)| self.component(i).join(part))
                    .collect(),
            ),
            (Self::Parts(_), Self::Reference { .. }) => other.join(self),
            (Self::Reference { targets, unknown_local }, other) | (other, Self::Reference { targets, unknown_local }) => {
                let mut joined = targets.clone();
                let unknown = if let Self::Reference {
                    targets: rhs,
                    unknown_local: unknown,
                } = other
                {
                    for target in rhs {
                        if !joined.contains(target) {
                            joined.push(target.clone());
                        }
                    }
                    *unknown
                } else {
                    Some(other.is_local())
                };
                Self::Reference {
                    targets: joined,
                    unknown_local: (*unknown_local).max(unknown),
                }
            }
            (Self::Caller, x) | (x, Self::Caller) => x.clone(),
            (Self::Parts(a), Self::Parts(b)) if a.len() == b.len() => Self::parts(a.iter().zip(b).map(|(a, b)| a.join(b)).collect()),
            _ => Self::Local,
        }
    }
}

/// The access and backing storage of a selected place or a pattern alias.
#[derive(Clone, Debug)]
pub(super) struct StoragePlace {
    /// Authority over the selected storage. A computed value can be lent
    /// temporarily, but it supplies no storage to write back to.
    pub(super) access: Option<Access>,
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
    pub(super) fn permits_inspection(
        &self,
        subject: &ValueType,
    ) -> bool {
        !matches!(self.place.as_ref().and_then(|p| p.access.as_ref()), Some(Access::Write)) && !matches!(subject.reference_parts(), Some((Access::Write, _)))
    }

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
        typer: &BodyTyper<'_, '_>,
        subject: &crate::ValueType,
        index: usize,
        count: usize,
    ) -> Result<Self, &'static str> {
        if let Some(part) = self.parts.get(index) {
            return Ok(part.clone());
        }
        let field = Component::Field { index, count };
        Ok(match subject.reference_parts() {
            Some((access, _)) => Self {
                value: typer.contents(&self.value).component(index),
                place: Some(StoragePlace {
                    access: Some(access.through(self.place.as_ref().and_then(|p| p.access.as_ref()))?),
                    backing: self.value.project(field),
                }),
                parts: Vec::new(),
            },
            _ => Self {
                value: self.value.component(index),
                place: self.place.clone().map(|mut p| {
                    p.backing = p.backing.project(field);
                    p
                }),
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

    /// Follow exactly the reference written at this occurrence. The caller
    /// decides whether to read, lend, or write the resulting place.
    pub(super) fn dereference_place(
        &mut self,
        node: usize,
    ) -> Result<(ValueType, Access), TypeError> {
        let operand = self.unit.single_child(node)?;
        let (reference, outer) = self.place_operand(operand)?;
        let (access, inner) = reference
            .reference_parts()
            .ok_or_else(|| TypeError::at(self.unit.file, operand, format!("dereference requires a reference, found {reference}")))?;
        let effective = access
            .through(outer.as_ref())
            .map_err(|m| TypeError::at(self.unit.file, node, m))?;
        let backing = self.origin(operand);
        self.record_place(node, operand, Some(access), None);
        self.set_origin(node, self.contents(&backing));
        self.places.insert(
            node,
            StoragePlace {
                access: Some(effective.clone()),
                backing,
            },
        );
        self.record_expression(node, inner.clone());
        Ok((inner.clone(), effective))
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
        // Groups nest as deeply as L1 admits; see :ExpressionLayers.
        if source.nodes[node_index].kind == NodeKind::Paren {
            let (inner, groups) = self.unit.grouped_operand(node_index)?;
            let (ty, borrowed) = self.place_operand(inner)?;
            for group in groups.into_iter().rev() {
                self.set_origin(group, self.origin(source.nodes[group].children[0]));
                self.record_expression(group, ty.clone());
            }
            return Ok((ty, borrowed));
        }
        if source.nodes[node_index].kind == NodeKind::Dereference {
            let (ty, access) = self.dereference_place(node_index)?;
            return Ok((ty, Some(access)));
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
            let (inner, groups) = self.unit.grouped_operand(node_index)?;
            let (ty, part) = self.pattern_subject(inner)?;
            for group in groups.into_iter().rev() {
                self.set_origin(group, part.value.clone());
                self.record_expression(group, ty.clone());
            }
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
            access: Some(access),
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
        access
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
        if let Some((_, place)) = self.borrowed_places.get(span) {
            self.contents(&place.backing)
        } else {
            self.binding_origins
                .get(&StorageRoot::Binding(*span))
                .cloned()
                .unwrap_or_default()
        }
    }

    pub(super) fn contents(
        &self,
        backing: &Origin,
    ) -> Origin {
        let Origin::Reference { targets, unknown_local } = backing else {
            return backing.whole();
        };
        let unknown = unknown_local.map(|local| if local { Origin::Local } else { Origin::Caller });
        targets
            .iter()
            .map(|target| {
                let root = self
                    .binding_origins
                    .get(&target.root)
                    .cloned()
                    .unwrap_or_else(|| target.root.initial_contents());
                target
                    .path
                    .iter()
                    .fold(root, |origin, component| match component {
                        Component::Field { index, .. } => origin.component(*index),
                        Component::Element => origin,
                    })
            })
            .chain(unknown)
            .reduce(|a, b| a.join(&b))
            .unwrap_or_default()
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
        self.binding_origins
            .insert(StorageRoot::Binding(span), source.value.clone());
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
        let Origin::Reference { targets, unknown_local } = self.storage_place(place).backing else {
            return;
        };
        let ambiguous = targets.len() != 1 || unknown_local.is_some();
        for target in targets {
            let root = self
                .binding_origins
                .entry(target.root)
                .or_insert_with_key(StorageRoot::initial_contents);
            let mut value = root;
            let mut weak = ambiguous;
            for component in target.path {
                match component {
                    Component::Element => weak = true,
                    Component::Field { index, count } => {
                        if let Origin::Case(_, fields) = value {
                            value = fields;
                        }
                        if !matches!(value, Origin::Parts(parts) if parts.len() == count) {
                            *value = Origin::Parts((0..count).map(|i| value.component(i)).collect());
                        }
                        let Origin::Parts(parts) = value else { unreachable!() };
                        value = &mut parts[index];
                    }
                }
            }
            *value = if weak { value.join(&stored) } else { stored.clone() };
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
                    access: Some(if self.writable.contains(span) { Access::Mut } else { Access::Shared }),
                    backing: Origin::reference(StorageRoot::Binding(*span)),
                });
        }
        let backing = match source.nodes[node].kind {
            NodeKind::PathExpression | NodeKind::Index => Origin::Caller,
            _ => Origin::Local,
        };
        StoragePlace { access: None, backing }
    }
}
