//! Universe syntax operations must preserve scope and bounds on default stacks.

use super::default_budget;
use crate::mathematical_core::{Context, CoreError, Level, Sort, Term, TermArena, infer_type};

const DEEP: usize = 131_072;

fn successor_chain(mut level: Level, count: usize) -> Level {
    for _ in 0..count {
        level = Level::Successor(Box::new(level));
    }
    level
}

#[test]
fn deep_universe_formation_and_scope_refusal_use_default_stack() {
    std::thread::spawn(|| {
        let mut arena = TermArena::new();
        let level = successor_chain(Level::Parameter(0), DEEP);
        let term = arena.insert(Term::Sort(Sort::Type(level)));
        eprintln!("constructed deep parametric universe; entering ordinary infer_type");
        let inferred = infer_type(
            &mut arena,
            &Context::with_level_arity(1),
            term,
            &mut default_budget(),
        )
        .expect("well-scoped universe formation");
        let Term::Sort(Sort::Type(mut level)) = arena.get(inferred) else {
            panic!("relevant universe")
        };
        let mut depth = 0;
        let mut cursor = &level;
        while let Level::Successor(child) = cursor {
            depth += 1;
            cursor = child;
        }
        assert_eq!(depth, DEEP + 1);
        assert!(matches!(cursor, Level::Parameter(0)));
        // The same syntax is not in scope under another universe context.
        assert_eq!(
            infer_type(&mut arena, &Context::empty(), term, &mut default_budget()),
            Err(CoreError::UnboundLevelParameter { index: 0, arity: 0 })
        );
        let maximum = arena.insert(Term::Sort(Sort::Type(Level::Constant(u32::MAX))));
        assert_eq!(
            infer_type(
                &mut arena,
                &Context::empty(),
                maximum,
                &mut default_budget()
            ),
            Err(CoreError::LevelOverflow)
        );
        // Retiring both deep arenas and owned cloned syntax is part of the
        // default-stack operation, including the previous refusal's cleanup.
        level = Level::Constant(0);
        assert_eq!(level, Level::Constant(0));
    })
    .join()
    .expect("universe formation and cleanup use the default stack");
}

#[test]
fn level_syntax_traits_keep_derived_hash_and_diagnostic_contracts() {
    use std::hash::{Hash, Hasher};
    #[derive(Debug, Hash)]
    enum Reference {
        Constant(u32),
        Parameter(u32),
        Successor(Box<Reference>),
        Maximum(Box<Reference>, Box<Reference>),
    }
    fn level(value: &Reference) -> Level {
        match value {
            Reference::Constant(value) => Level::Constant(*value),
            Reference::Parameter(index) => Level::Parameter(*index),
            Reference::Successor(inner) => Level::Successor(Box::new(level(inner))),
            Reference::Maximum(left, right) => {
                Level::Maximum(Box::new(level(left)), Box::new(level(right)))
            }
        }
    }
    fn hash(value: &impl Hash) -> u64 {
        let mut state = std::hash::DefaultHasher::new();
        value.hash(&mut state);
        state.finish()
    }
    for reference in [
        Reference::Constant(0),
        Reference::Constant(u32::MAX),
        Reference::Parameter(0),
        Reference::Successor(Box::new(Reference::Parameter(3))),
        Reference::Maximum(
            Box::new(Reference::Parameter(0)),
            Box::new(Reference::Successor(Box::new(Reference::Constant(7)))),
        ),
    ] {
        let actual = level(&reference);
        assert_eq!(format!("{actual:?}"), format!("{reference:?}"));
        assert_eq!(format!("{actual:#?}"), format!("{reference:#?}"));
        assert_eq!(hash(&actual), hash(&reference));
        assert_eq!(actual, actual.clone());
    }
}

#[test]
fn deep_level_operations_preserve_syntax_algebra_and_first_refusal() {
    std::thread::spawn(|| {
        use crate::mathematical_core::term::{instantiate_level, levels_equal};
        use std::hash::{Hash, Hasher};
        let level = successor_chain(Level::Parameter(0), DEEP);
        let clone = level.clone();
        assert_eq!(level, clone);
        let mut first = std::hash::DefaultHasher::new();
        level.hash(&mut first);
        let mut second = std::hash::DefaultHasher::new();
        clone.hash(&mut second);
        assert_eq!(first.finish(), second.finish());
        assert_eq!(
            format!("{level:?}").len(),
            DEEP * "Successor()".len() + "Parameter(0)".len()
        );
        let absorbed = level.clone().maximum(Level::Constant(DEEP as u32));
        assert_ne!(
            level, absorbed,
            "syntactic equality must not become semantic conversion"
        );
        assert!(levels_equal(&level, &absorbed));
        let different = level.clone().maximum(Level::Constant(DEEP as u32 + 1));
        assert!(!levels_equal(&level, &different));
        let closed = instantiate_level(&level, &[Level::Constant(1)]).unwrap();
        assert!(levels_equal(&closed, &Level::Constant(DEEP as u32 + 1)));
        assert!(
            matches!(closed, Level::Successor(_)),
            "instantiation preserves explicit syntax for wire-depth admission"
        );
        let missing = Level::Maximum(Box::new(level.clone()), Box::new(Level::Parameter(1)));
        assert_eq!(instantiate_level(&missing, &[Level::Constant(0)]), Err(1));
        assert_eq!(
            instantiate_level(&missing, &[]),
            Err(0),
            "left-to-right missing-parameter precedence"
        );
    })
    .join()
    .expect("all level operations and refusal cleanup use the default stack");
}
