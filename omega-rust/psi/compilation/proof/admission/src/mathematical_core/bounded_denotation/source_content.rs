//! Source content values use the same opaque canonical algebra carriers as
//! checked content derivations. A carrier declaration forms values and Id
//! statements; it grants no conservation theorem or resource authority.

use super::{BoundedDenotationError, Denotation};
use crate::mathematical_core::{Declaration, Level, TermArena, TermHandle};
use semantic_vocabulary::ContentAlgebraKind;

/// One arena's append-only content vocabulary. Stored handles belong to that
/// arena/signature pair, just as they do in SourceIntegerTheory.
pub struct SourceContentTheory {
    state: Denotation,
}

impl SourceContentTheory {
    pub fn new(arena: &mut TermArena, declarations: &mut Vec<Declaration>) -> Self {
        let mut state = Denotation::new_in(std::mem::take(arena), std::mem::take(declarations));
        std::mem::swap(arena, &mut state.arena);
        std::mem::swap(declarations, &mut state.declarations);
        Self { state }
    }

    pub fn carrier(
        &mut self,
        arena: &mut TermArena,
        declarations: &mut Vec<Declaration>,
        kind: ContentAlgebraKind,
        parameter: TermHandle,
        level: Level,
    ) -> Result<TermHandle, BoundedDenotationError> {
        std::mem::swap(arena, &mut self.state.arena);
        std::mem::swap(declarations, &mut self.state.declarations);
        let result = self.state.content_carrier_at(kind, parameter, level);
        std::mem::swap(arena, &mut self.state.arena);
        std::mem::swap(declarations, &mut self.state.declarations);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Budget, Context, DEFAULT_CONVERSION_STEPS, Sort, Term, check_signature, check_type,
        convertible,
    };

    #[test]
    fn canonical_content_family_substitutes_its_outer_type_without_storing_it() {
        let mut arena = TermArena::default();
        let mut declarations = Vec::new();
        let mut content = SourceContentTheory::new(&mut arena, &mut declarations);
        let type_zero = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
        let parameter = arena.insert(Term::Variable(0));
        let generic = content
            .carrier(
                &mut arena,
                &mut declarations,
                ContentAlgebraKind::CountedQuantity,
                parameter,
                Level::Constant(0),
            )
            .unwrap();
        let family = arena.insert(Term::Lambda {
            domain: type_zero,
            body: generic,
        });
        let two = arena.insert(Term::Two);
        let applied = arena.insert(Term::Apply {
            function: family,
            argument: two,
        });
        let concrete = content
            .carrier(
                &mut arena,
                &mut declarations,
                ContentAlgebraKind::CountedQuantity,
                two,
                Level::Constant(0),
            )
            .unwrap();
        let higher = content
            .carrier(
                &mut arena,
                &mut declarations,
                ContentAlgebraKind::CountedQuantity,
                type_zero,
                Level::Constant(1),
            )
            .unwrap();
        let intervals = content
            .carrier(
                &mut arena,
                &mut declarations,
                ContentAlgebraKind::IntervalSet,
                two,
                Level::Constant(0),
            )
            .unwrap();
        let signature = check_signature(
            &mut arena,
            &declarations,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap();
        let context = Context::empty().with_signature(signature);
        for value in [applied, concrete, higher, intervals] {
            check_type(
                &mut arena,
                &context,
                value,
                type_zero,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
            )
            .unwrap();
        }
        assert!(
            convertible(
                &mut arena,
                &context,
                applied,
                concrete,
                type_zero,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS)
            )
            .unwrap()
        );
        assert!(
            !convertible(
                &mut arena,
                &context,
                concrete,
                intervals,
                type_zero,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS)
            )
            .unwrap()
        );
    }
}
