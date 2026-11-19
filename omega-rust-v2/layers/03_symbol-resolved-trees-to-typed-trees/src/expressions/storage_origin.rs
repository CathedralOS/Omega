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
    /// Whose storage the field lives in: the borrowed subject's origin.
    pub(super) backing: Origin,
}

/// What a pattern position receives: the origin of the value it matches and,
/// when that value lives behind a reference, the borrowed place it names.
#[derive(Clone, Debug, Default)]
pub(super) struct PatternSource {
    pub(super) value: Origin,
    pub(super) place: Option<BorrowedPlace>,
}

impl PatternSource {
    pub(super) fn of(value: Origin) -> Self {
        Self { value, place: None }
    }
    /// The source of one component of a subject of type `subject`. Fields of
    /// a referenced value live in the reference's backing.
    pub(super) fn component(
        &self,
        subject: &crate::ValueType,
        index: usize,
    ) -> Self {
        let reference = match subject {
            crate::ValueType::AtLifetime(reference, _) => reference.as_ref(),
            other => other,
        };
        match reference {
            crate::ValueType::Reference(..) => Self {
                value: self.value.whole(),
                place: Some(BorrowedPlace { backing: self.value.whole() }),
            },
            _ => Self {
                value: self.value.component(index),
                place: self.place.clone(),
            },
        }
    }
}

impl BodyTyper<'_, '_> {
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
