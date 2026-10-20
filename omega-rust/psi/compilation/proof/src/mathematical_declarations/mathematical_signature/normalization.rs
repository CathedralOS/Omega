//! Reduce prepared applications through the existing checked signature once.
//! Source consumers borrow the resulting arena; they neither repeat formation
//! nor allocate a private copy of the signature to perform substitution.

use proof_admission::{
    Budget, CoreError, Signature, Term, TermArena, TermHandle, weak_head_normalize_retaining,
};

pub(super) fn normalize(
    arena: &mut TermArena,
    signature: &Signature,
    term: TermHandle,
    budget: &mut Budget,
    retain: &impl Fn(u32) -> bool,
) -> Result<TermHandle, CoreError> {
    let head = weak_head_normalize_retaining(arena, signature, term, budget, retain)?;
    let original = arena.get(head);
    macro_rules! children {
        ($variant:ident { $($field:ident),+ }) => {
            Term::$variant { $($field: normalize(arena, signature, $field, budget, retain)?),+ }
        };
    }
    let reduced = match original.clone() {
        Term::Pi { domain, codomain } => children!(Pi { domain, codomain }),
        Term::Lambda { domain, body } => children!(Lambda { domain, body }),
        Term::Apply { function, argument } => children!(Apply { function, argument }),
        Term::Sigma { domain, codomain } => children!(Sigma { domain, codomain }),
        Term::Pair { first, second } => children!(Pair { first, second }),
        Term::Fst { pair } => children!(Fst { pair }),
        Term::Snd { pair } => children!(Snd { pair }),
        Term::CaseTwo {
            motive,
            zero_branch,
            one_branch,
            scrutinee,
        } => children!(CaseTwo {
            motive,
            zero_branch,
            one_branch,
            scrutinee
        }),
        Term::Id { ty, left, right } => children!(Id { ty, left, right }),
        Term::Refl { ty, value } => children!(Refl { ty, value }),
        Term::IdElim {
            motive,
            base,
            endpoint,
            proof,
        } => children!(IdElim {
            motive,
            base,
            endpoint,
            proof
        }),
        Term::W { carrier, children } => children!(W { carrier, children }),
        Term::Sup {
            carrier,
            children,
            label,
            function,
        } => children!(Sup {
            carrier,
            children,
            label,
            function
        }),
        Term::IndW { motive, step, tree } => children!(IndW { motive, step, tree }),
        Term::EmptyElim { ty, scrutinee } => children!(EmptyElim { ty, scrutinee }),
        Term::Squash { ty } => children!(Squash { ty }),
        Term::SquashIntro { ty, value } => children!(SquashIntro { ty, value }),
        Term::SquashElim {
            proposition,
            function,
            scrutinee,
        } => children!(SquashElim {
            proposition,
            function,
            scrutinee
        }),
        Term::Box { ty } => children!(Box { ty }),
        Term::BoxIntro { ty, value } => children!(BoxIntro { ty, value }),
        Term::BoxElim {
            motive,
            body,
            scrutinee,
        } => children!(BoxElim {
            motive,
            body,
            scrutinee
        }),
        leaf => leaf,
    };
    Ok(if reduced == original {
        head
    } else {
        arena.insert(reduced)
    })
}
