//! Complete structural comparison with spelling supplied by each forest owner.
//! Unlike root snapshots, this includes detached nodes and arena allocation
//! metadata: Build continuation sharing must compare all remaining source work.
use crate::{SyntaxTextOwner, SyntaxTrees};
use arena::{Arena, Handle, HandleSpan, PagedArenaSet};
use std::sync::Arc;

pub trait SyntaxEquivalent {
    fn same_syntax(
        &self,
        other: &Self,
        left_text: &SyntaxTextOwner,
        right_text: &SyntaxTextOwner,
    ) -> bool;
}

impl SyntaxEquivalent for crate::identifier::Identifier {
    fn same_syntax(
        &self,
        other: &Self,
        left_text: &SyntaxTextOwner,
        right_text: &SyntaxTextOwner,
    ) -> bool {
        // Identifier equality has always compared spelling, independently of
        // source coordinates. Coordinates remain separately compared wherever
        // the enclosing syntax representation carries them.
        self.as_str(left_text) == other.as_str(right_text)
    }
}
impl PartialEq for SyntaxTrees {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            source_id,
            roots,
            tables,
        } = self;
        let crate::SyntaxTreeTables {
            text,
            items,
            expressions,
            statements,
            type_references,
        } = tables;
        *source_id == other.source_id
            && *roots == other.roots
            && items.same_syntax(&other.tables.items, text, &other.text)
            && expressions.same_syntax(&other.tables.expressions, text, &other.text)
            && statements.same_syntax(&other.tables.statements, text, &other.text)
            && type_references.same_syntax(&other.tables.type_references, text, &other.text)
    }
}
impl Eq for SyntaxTrees {}

impl<T> SyntaxEquivalent for Handle<T> {
    fn same_syntax(&self, other: &Self, _: &SyntaxTextOwner, _: &SyntaxTextOwner) -> bool {
        self == other
    }
}
impl<T> SyntaxEquivalent for HandleSpan<T> {
    fn same_syntax(&self, other: &Self, _: &SyntaxTextOwner, _: &SyntaxTextOwner) -> bool {
        self == other
    }
}
impl<T: SyntaxEquivalent> SyntaxEquivalent for Vec<T> {
    fn same_syntax(&self, other: &Self, left: &SyntaxTextOwner, right: &SyntaxTextOwner) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(left_value, right_value)| left_value.same_syntax(right_value, left, right))
    }
}
impl<T: SyntaxEquivalent> SyntaxEquivalent for Option<T> {
    fn same_syntax(&self, other: &Self, left: &SyntaxTextOwner, right: &SyntaxTextOwner) -> bool {
        match (self, other) {
            (Some(left_value), Some(right_value)) => {
                left_value.same_syntax(right_value, left, right)
            }
            (None, None) => true,
            _ => false,
        }
    }
}
macro_rules! indirect {
    ($($container:ident),*) => { $(impl<T: SyntaxEquivalent + ?Sized> SyntaxEquivalent for $container<T> {
        fn same_syntax(&self, other: &Self, left: &SyntaxTextOwner, right: &SyntaxTextOwner) -> bool { (**self).same_syntax(&**other,left,right) }
    })* };
}
indirect!(Box, Arc);
impl<A: SyntaxEquivalent, B: SyntaxEquivalent> SyntaxEquivalent for (A, B) {
    fn same_syntax(&self, other: &Self, left: &SyntaxTextOwner, right: &SyntaxTextOwner) -> bool {
        self.0.same_syntax(&other.0, left, right) && self.1.same_syntax(&other.1, left, right)
    }
}
macro_rules! arena_equality {
    ($($arena:ident),*) => { $(impl<T: Default + SyntaxEquivalent> SyntaxEquivalent for $arena<T> {
        fn same_syntax(&self, other: &Self, left: &SyntaxTextOwner, right: &SyntaxTextOwner) -> bool {
            self.equal_by(other, |left_value,right_value| left_value.same_syntax(right_value,left,right))
        }
    })* };
}
arena_equality!(Arena, PagedArenaSet);
macro_rules! plain {
    ($($kind:ty),* $(,)?) => { $(impl SyntaxEquivalent for $kind {
        fn same_syntax(&self, other: &Self, _: &SyntaxTextOwner, _: &SyntaxTextOwner) -> bool { self == other }
    })* };
}
plain!(
    bool,
    usize,
    u8,
    u16,
    u32,
    u64,
    i64,
    String,
    source::SourceSpan,
    source::Span,
    source::SourceId,
    source::SourceBytes
);

impl<T: SyntaxEquivalent> SyntaxEquivalent for [T] {
    fn same_syntax(&self, other: &Self, left: &SyntaxTextOwner, right: &SyntaxTextOwner) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other)
                .all(|(left_value, right_value)| left_value.same_syntax(right_value, left, right))
    }
}
plain!(
    language_core::OperatorSpelling,
    numerics::literals::FloatFormat,
    numerics::literals::IntegerLiteral
);
plain!(
    language_core::LocalBindingRole,
    language_core::BindingRelevance,
    language_core::DataSupplyMode,
    language_core::CallOperationalAcknowledgement,
    language_core::DomainPredicateBody,
    language_core::ReferenceAccess,
    language_core::DomainClassification,
    numerics::arithmetic::ArithmeticDomain
);
