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
use crate::{Access, Site, TypeError, TypedExpression, ValueType};
use omega_lexer::SourceSpan;
use omega_parser::NodeKind;
use omega_resolver::Symbol;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) enum Origin {
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
            Self::Parts(parts) => parts.get(index).cloned().unwrap_or(Self::Local),
            other => other.clone(),
        }
    }
    pub(super) fn join(
        &self,
        other: &Self,
    ) -> Self {
        match (self, other) {
            (Self::Caller, x) | (x, Self::Caller) => x.clone(),
            (Self::Parts(a), Self::Parts(b)) if a.len() == b.len() => Self::parts(a.iter().zip(b).map(|(a, b)| a.join(b)).collect()),
            _ => Self::Local,
        }
    }
}

/// A binding that names a field of borrowed storage in place.
#[derive(Clone, Debug)]
pub(super) struct BorrowedPlace {
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
    pub(super) place: Option<BorrowedPlace>,
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
    ) -> Self {
        if let Some(part) = self.parts.get(index) {
            return part.clone();
        }
        let reference = match subject {
            crate::ValueType::AtLifetime(reference, _) => reference.as_ref(),
            other => other,
        };
        match reference {
            crate::ValueType::Reference(access, _) => Self {
                value: self.value.whole(),
                place: Some(BorrowedPlace {
                    access: self
                        .place
                        .as_ref()
                        .map_or(access.clone(), |outer| weaker(&outer.access, access)),
                    backing: self.value.whole(),
                }),
                parts: Vec::new(),
            },
            _ => Self {
                value: self.value.component(index),
                place: self.place.clone(),
                parts: Vec::new(),
            },
        }
    }
}

/// The access a place keeps when it is reached through two references in turn:
/// a `@mut` reached through `@` only reads, and nothing reads through `@write`.
pub(super) fn weaker(
    outer: &Access,
    inner: &Access,
) -> Access {
    match (outer, inner) {
        (Access::Write, _) | (_, Access::Write) => Access::Write,
        (Access::Shared, _) | (_, Access::Shared) => Access::Shared,
        _ => Access::Mut,
    }
}

impl BodyTyper<'_, '_> {
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
            self.output.expressions.push(TypedExpression {
                node: source.node_id(node_index),
                site: Site::at(source, node_index),
                ty: ty.clone(),
            });
            return Ok((ty, borrowed));
        }
        if source.nodes[node_index].kind == NodeKind::FieldAccess {
            return self.project_fields(node_index, &Access::Shared);
        }
        if source.nodes[node_index].kind == NodeKind::PathExpression
            && let Some(Symbol::Binding { span, .. }) = self.unit.symbol(node_index)
            && let Some(place) = self.borrowed_places.get(span)
        {
            let access = place.access.clone();
            let ty = self
                .locals
                .get(span)
                .cloned()
                .ok_or_else(|| TypeError::at(source, node_index, "missing type of resolved binding"))?;
            self.set_origin(node_index, self.binding_origin(span));
            self.output.expressions.push(TypedExpression {
                node: source.node_id(node_index),
                site: Site::at(source, node_index),
                ty: ty.clone(),
            });
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
            self.output.expressions.push(TypedExpression {
                node: source.node_id(node_index),
                site: Site::at(source, node_index),
                ty: ty.clone(),
            });
            return Ok((ty, PatternSource { value, place: None, parts }));
        }
        let (ty, borrowed) = self.place_operand(node_index)?;
        let place = borrowed.map(|access| BorrowedPlace {
            access,
            backing: self.backing(node_index),
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
        &self,
        span: &SourceSpan,
    ) -> Option<Access> {
        self.borrowed_places.get(span).map(|place| place.access.clone())
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
        source: &PatternSource,
    ) {
        self.set_binding_origin(span, source.value.clone());
        match &source.place {
            Some(place) => self.borrowed_places.insert(span, place.clone()),
            None => self.borrowed_places.remove(&span),
        };
    }
    /// Whose storage a typed place denotes. Field selection and indexing
    /// record their backing when they type the place.
    pub(super) fn backing(
        &self,
        node_index: usize,
    ) -> Origin {
        let source = self.unit.file;
        match source.nodes[node_index].kind {
            NodeKind::Paren => self
                .unit
                .single_child(node_index)
                .map_or(Origin::Local, |child| self.backing(child)),
            NodeKind::FieldAccess | NodeKind::Index => self.backings.get(&node_index).cloned().unwrap_or_default(),
            NodeKind::PathExpression => match self.unit.symbol(node_index) {
                Some(Symbol::Binding { span, .. }) => self
                    .borrowed_places
                    .get(span)
                    .map_or(Origin::Local, |place| place.backing.clone()),
                // Declarations name storage this invocation does not own.
                _ => Origin::Caller,
            },
            // Every other operand is a temporary of this invocation.
            _ => Origin::Local,
        }
    }
    pub(super) fn set_backing(
        &mut self,
        node_index: usize,
        backing: Origin,
    ) {
        if backing == Origin::Caller {
            self.backings.remove(&node_index);
        } else {
            self.backings.insert(node_index, backing);
        }
    }
}
