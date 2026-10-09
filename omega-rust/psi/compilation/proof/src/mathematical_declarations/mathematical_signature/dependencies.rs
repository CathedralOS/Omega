//! Opaque source values depend on exact bound inputs. Prune the telescope
//! through its dependency closure, then use ordinary capture-avoiding kernel
//! substitution to rebind domain indices. Dummy arguments occupy only unused
//! slots and disappear before the resulting declaration is kernel-checked.

use proof_admission::{Term, TermArena, TermHandle, substitute};

pub(super) fn free_positions(
    arena: &TermArena,
    term: TermHandle,
    scope: usize,
    positions: &mut Vec<usize>,
) {
    fn walk(
        arena: &TermArena,
        term: TermHandle,
        scope: usize,
        depth: u32,
        positions: &mut Vec<usize>,
    ) {
        macro_rules! children { ($($child:ident),+) => {{ $(walk(arena, $child, scope, depth, positions);)+ }}; }
        match arena.get(term) {
            Term::Variable(index) if index >= depth => {
                if let Some(position) = scope.checked_sub((index - depth) as usize + 1) {
                    if !positions.contains(&position) {
                        positions.push(position);
                    }
                }
            }
            Term::Pi { domain, codomain } | Term::Sigma { domain, codomain } => {
                walk(arena, domain, scope, depth, positions);
                walk(arena, codomain, scope, depth + 1, positions);
            }
            Term::Lambda { domain, body } => {
                walk(arena, domain, scope, depth, positions);
                walk(arena, body, scope, depth + 1, positions);
            }
            Term::Apply { function, argument } => children!(function, argument),
            Term::Pair { first, second } => children!(first, second),
            Term::Fst { pair } | Term::Snd { pair } => children!(pair),
            Term::CaseTwo {
                motive,
                zero_branch,
                one_branch,
                scrutinee,
            } => children!(motive, zero_branch, one_branch, scrutinee),
            Term::Id { ty, left, right } => children!(ty, left, right),
            Term::Refl { ty, value }
            | Term::SquashIntro { ty, value }
            | Term::BoxIntro { ty, value } => children!(ty, value),
            Term::IdElim {
                motive,
                base,
                endpoint,
                proof,
            } => children!(motive, base, endpoint, proof),
            Term::W { carrier, children } => children!(carrier, children),
            Term::Sup {
                carrier,
                children,
                label,
                function,
            } => children!(carrier, children, label, function),
            Term::IndW { motive, step, tree } => children!(motive, step, tree),
            Term::EmptyElim { ty, scrutinee } => children!(ty, scrutinee),
            Term::Squash { ty } | Term::Box { ty } => children!(ty),
            Term::SquashElim {
                proposition,
                function,
                scrutinee,
            } => children!(proposition, function, scrutinee),
            Term::BoxElim {
                motive,
                body,
                scrutinee,
            } => children!(motive, body, scrutinee),
            _ => {}
        }
    }
    walk(arena, term, scope, 0, positions);
}

pub(super) fn rebind_domain(
    arena: &mut TermArena,
    domain: TermHandle,
    old_domains: &[TermHandle],
    retained: &[usize],
) -> TermHandle {
    let mut body = domain;
    for domain in old_domains.iter().rev() {
        body = arena.insert(Term::Lambda {
            domain: *domain,
            body,
        });
    }
    for position in 0..old_domains.len() {
        let argument = retained
            .iter()
            .position(|retained| *retained == position)
            .map(|index| arena.insert(Term::Variable((retained.len() - 1 - index) as u32)))
            .unwrap_or_else(|| arena.insert(Term::Dummy));
        let Term::Lambda { body: inner, .. } = arena.get(body) else {
            unreachable!("constructed telescope")
        };
        body = substitute(arena, inner, argument);
    }
    body
}
