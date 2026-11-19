use arena::{Handle, HandleSpan};
use source_text::{SourceId, SourceSpan};
use std::sync::Arc;

/// The private coordinates of a standalone unit page and the admitted
/// coordinates it moves to.
#[derive(Debug, Clone, Copy)]
pub struct PageRelocation {
    pub(crate) from_generation: u32,
    pub(crate) generation: u32,
    pub(crate) from_source: SourceId,
    pub(crate) source: SourceId,
}

pub trait SyntaxRelocate {
    fn relocate(&mut self, relocation: &PageRelocation);
}

impl<T> SyntaxRelocate for Handle<T> {
    fn relocate(&mut self, relocation: &PageRelocation) {
        if self.generation() == relocation.from_generation {
            *self = Handle::from_parts(self.arena_index(), relocation.generation);
        }
    }
}
impl<T> SyntaxRelocate for HandleSpan<T> {
    fn relocate(&mut self, relocation: &PageRelocation) {
        let mut start = self.start();
        start.relocate(relocation);
        *self = HandleSpan::from_parts_or_empty(start, self.count());
    }
}
impl SyntaxRelocate for SourceId {
    fn relocate(&mut self, relocation: &PageRelocation) {
        if *self == relocation.from_source {
            *self = relocation.source;
        }
    }
}
impl SyntaxRelocate for SourceSpan {
    fn relocate(&mut self, relocation: &PageRelocation) {
        self.source_id.relocate(relocation);
    }
}
impl<T: SyntaxRelocate> SyntaxRelocate for Vec<T> {
    fn relocate(&mut self, relocation: &PageRelocation) {
        self.as_mut_slice().relocate(relocation);
    }
}
impl<T: SyntaxRelocate> SyntaxRelocate for [T] {
    fn relocate(&mut self, relocation: &PageRelocation) {
        self.iter_mut().for_each(|value| value.relocate(relocation));
    }
}
impl<T: SyntaxRelocate, const N: usize> SyntaxRelocate for [T; N] {
    fn relocate(&mut self, relocation: &PageRelocation) {
        self.as_mut_slice().relocate(relocation);
    }
}
impl<T: SyntaxRelocate> SyntaxRelocate for Option<T> {
    fn relocate(&mut self, relocation: &PageRelocation) {
        if let Some(value) = self {
            value.relocate(relocation);
        }
    }
}
impl<T: SyntaxRelocate + ?Sized> SyntaxRelocate for Box<T> {
    fn relocate(&mut self, relocation: &PageRelocation) {
        (**self).relocate(relocation);
    }
}
impl<T: SyntaxRelocate + Clone> SyntaxRelocate for Arc<T> {
    fn relocate(&mut self, relocation: &PageRelocation) {
        Arc::make_mut(self).relocate(relocation);
    }
}
impl<A: SyntaxRelocate, B: SyntaxRelocate> SyntaxRelocate for (A, B) {
    fn relocate(&mut self, relocation: &PageRelocation) {
        self.0.relocate(relocation);
        self.1.relocate(relocation);
    }
}
macro_rules! coordinate_free {
        ($($kind:ty),* $(,)?) => { $(impl SyntaxRelocate for $kind {
            fn relocate(&mut self, _: &PageRelocation) {}
        })* };
    }
coordinate_free!(
    bool,
    usize,
    u8,
    u16,
    u32,
    u64,
    i64,
    String,
    source_text::Span,
    source_text::SourceBytes,
    language_core::PatternComponent,
    language_core::OperatorSpelling,
    numeric_literals::FloatFormat,
    numeric_literals::IntegerLiteral,
    language_core::LocalBindingRole,
    language_core::BindingRelevance,
    language_core::DataSupplyMode,
    language_core::CallOperationalAcknowledgement,
    language_core::DomainPredicatePresence,
    language_core::ReferenceAccess,
    language_core::DomainClassification,
    numerics::arithmetic::ArithmeticDomain,
    language_core::inline_assembly::AsmTargetApplicability,
    language_core::atomic::AtomicExpressionResultCustody,
    language_core::atomic::AtomicOrderingPlan,
    language_core::CarryPolicy,
    language_core::CastForm,
    numeric_literals::EvaluatedConstant,
    language_core::Multiplicity,
);
