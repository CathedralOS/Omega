//! Generated theorem definitions bind their source parameters so temporary
//! assumptions cannot escape. Parameterized copies preserve the immutable
//! signature prefix and reuse shared definitions; dependent assumptions remain
//! ungranted.

use proof_admission::{Declaration, Term, TermArena, TermHandle};
use symbols::SymbolHandle;

/// One immutable source telescope's copies in its generated declaration
/// fragment. Coordinates before this base cannot depend on the telescope.
#[derive(Default)]
pub(in super::super) struct ParameterAbstractions {
    parameters: Vec<(u32, TermHandle)>,
    base: usize,
    copies: Vec<Option<u32>>,
}
impl ParameterAbstractions {
    fn get(&self, position: u32) -> Option<u32> {
        (position as usize)
            .checked_sub(self.base)
            .and_then(|position| self.copies.get(position))
            .copied()
            .flatten()
    }
}

/// Abstract temporary parameter assumptions without capturing existing binders.
pub(super) fn abstract_parameters(
    arena: &mut TermArena,
    proof: TermHandle,
    ty: TermHandle,
    parameters: &[(u32, SymbolHandle, TermHandle)],
) -> Result<(TermHandle, TermHandle), &'static str> {
    abstract_parameters_with_declarations(
        arena,
        proof,
        ty,
        parameters,
        &ParameterAbstractions::default(),
    )
}

fn abstract_parameters_with_declarations(
    arena: &mut TermArena,
    proof: TermHandle,
    ty: TermHandle,
    parameters: &[(u32, SymbolHandle, TermHandle)],
    lifted: &ParameterAbstractions,
) -> Result<(TermHandle, TermHandle), &'static str> {
    fn go(
        arena: &mut TermArena,
        term: TermHandle,
        parameters: &[(u32, SymbolHandle, TermHandle)],
        lifted: &ParameterAbstractions,
        depth: u32,
    ) -> Result<TermHandle, &'static str> {
        macro_rules! at {
            ($value:expr) => {
                go(arena, $value, parameters, lifted, depth)?
            };
        }
        macro_rules! under {
            ($value:expr) => {
                go(arena, $value, parameters, lifted, depth + 1)?
            };
        }
        let node = match arena.get(term) {
            Term::Constant {
                declaration,
                levels,
            } => {
                if let Some(ordinal) = parameters
                    .iter()
                    .position(|(position, _, _)| *position == declaration)
                {
                    let arity =
                        proof_admission::universe_parameter_arity(arena, &[parameters[ordinal].2]);
                    let expected = (0..arity)
                        .map(proof_admission::Level::Parameter)
                        .collect::<Vec<_>>();
                    if levels != expected {
                        return Err(
                            "source value placeholder changed its exact universe application",
                        );
                    }
                    Term::Variable(depth + (parameters.len() - ordinal - 1) as u32)
                } else if let Some(replacement) = lifted.get(declaration) {
                    let mut term = arena.insert(Term::Constant {
                        declaration: replacement,
                        levels,
                    });
                    for ordinal in 0..parameters.len() {
                        let argument = arena.insert(Term::Variable(
                            depth + (parameters.len() - ordinal - 1) as u32,
                        ));
                        term = arena.insert(Term::Apply {
                            function: term,
                            argument,
                        });
                    }
                    return Ok(term);
                } else {
                    return Ok(term);
                }
            }
            Term::Variable(index) => {
                if index >= depth {
                    return Err("source case proof contains an unbound value before abstraction");
                }
                return Ok(term);
            }
            Term::Pi { domain, codomain } => term!(Pi => at!(domain) => under!(codomain)),
            Term::Lambda { domain, body } => term!(Lambda => at!(domain) => under!(body)),
            Term::Sigma { domain, codomain } => term!(Sigma => at!(domain) => under!(codomain)),
            Term::Apply { function, argument } => term!(Apply => at!(function) => at!(argument)),
            Term::Pair { first, second } => term!(Pair => at!(first) => at!(second)),
            Term::Fst { pair } => Term::Fst { pair: at!(pair) },
            Term::Snd { pair } => Term::Snd { pair: at!(pair) },
            Term::Id { ty, left, right } => Term::Id {
                ty: at!(ty),
                left: at!(left),
                right: at!(right),
            },
            Term::Refl { ty, value } => term!(Refl => at!(ty) => at!(value)),
            Term::IdElim {
                motive,
                base,
                endpoint,
                proof,
            } => Term::IdElim {
                motive: at!(motive),
                base: at!(base),
                endpoint: at!(endpoint),
                proof: at!(proof),
            },
            Term::CaseTwo {
                motive,
                zero_branch,
                one_branch,
                scrutinee,
            } => Term::CaseTwo {
                motive: at!(motive),
                zero_branch: at!(zero_branch),
                one_branch: at!(one_branch),
                scrutinee: at!(scrutinee),
            },
            Term::Box { ty } => Term::Box { ty: at!(ty) },
            Term::BoxIntro { ty, value } => Term::BoxIntro {
                ty: at!(ty),
                value: at!(value),
            },
            Term::BoxElim {
                motive,
                body,
                scrutinee,
            } => Term::BoxElim {
                motive: at!(motive),
                body: at!(body),
                scrutinee: at!(scrutinee),
            },
            Term::EmptyElim { ty, scrutinee } => Term::EmptyElim {
                ty: at!(ty),
                scrutinee: at!(scrutinee),
            },
            Term::W { carrier, children } => Term::W {
                carrier: at!(carrier),
                children: at!(children),
            },
            Term::Sup {
                carrier,
                children,
                label,
                function,
            } => Term::Sup {
                carrier: at!(carrier),
                children: at!(children),
                label: at!(label),
                function: at!(function),
            },
            Term::IndW { motive, step, tree } => Term::IndW {
                motive: at!(motive),
                step: at!(step),
                tree: at!(tree),
            },
            Term::Squash { ty } => Term::Squash { ty: at!(ty) },
            Term::SquashIntro { ty, value } => Term::SquashIntro {
                ty: at!(ty),
                value: at!(value),
            },
            Term::SquashElim {
                proposition,
                function,
                scrutinee,
            } => Term::SquashElim {
                proposition: at!(proposition),
                function: at!(function),
                scrutinee: at!(scrutinee),
            },
            Term::Dummy => return Err("source case proof contains a dummy term"),
            Term::Sort(_) | Term::Two | Term::TwoZero | Term::TwoOne | Term::Empty => {
                return Ok(term);
            }
        };
        Ok(arena.insert(node))
    }
    let mut body = go(arena, proof, parameters, lifted, 0)?;
    let mut statement = go(arena, ty, parameters, lifted, 0)?;
    for (ordinal, &(_, _, domain)) in parameters.iter().enumerate().rev() {
        // A child/premise domain may depend on earlier value parameters. Its
        // binder is formed under that prefix, not under the full body scope.
        let domain = go(arena, domain, &parameters[..ordinal], lifted, 0)?;
        body = arena.insert(Term::Lambda { domain, body });
        statement = arena.insert(term!(Pi => domain => statement));
    }
    Ok((body, statement))
}

pub(super) fn excludes_parameters(
    arena: &TermArena,
    declarations: &[Declaration],
    roots: &[TermHandle],
    parameters: &[(u32, SymbolHandle, TermHandle)],
) -> bool {
    let mut seen_terms = Vec::new();
    let Some(references) = collect_constant_references(arena, roots, &mut seen_terms) else {
        return false;
    };
    let mut pending = references.into_iter().collect::<Vec<_>>();
    let mut seen = Vec::<u64>::new();
    while let Some(position) = pending.pop() {
        if parameters
            .iter()
            .any(|(parameter, _, _)| *parameter == position)
        {
            return false;
        }
        let Some(declaration) = declarations.get(position as usize) else {
            return false;
        };
        let word = position as usize / 64;
        if word >= seen.len() {
            seen.resize(word + 1, 0);
        }
        let bit = 1u64 << (position % 64);
        if seen[word] & bit != 0 {
            continue;
        }
        seen[word] |= bit;
        let roots = std::iter::once(declaration.ty)
            .chain(declaration.body)
            .collect::<Vec<_>>();
        let Some(references) = collect_constant_references(arena, &roots, &mut seen_terms) else {
            return false;
        };
        pending.extend(references);
    }
    true
}

/// Close an outer source telescope through shared generated definitions.
/// Dependent definitions are copied once as parameterized definitions; existing
/// declarations/caches remain immutable and no assumption gains authority.
pub(in super::super) fn abstract_parameter_closure(
    arena: &mut TermArena,
    declarations: &mut Vec<Declaration>,
    proof: TermHandle,
    ty: TermHandle,
    parameters: &[(u32, SymbolHandle, TermHandle)],
    lifted: &mut ParameterAbstractions,
) -> Result<(TermHandle, TermHandle), &'static str> {
    if parameters.is_empty() {
        return Ok((proof, ty));
    }
    let original_len = declarations.len();
    let first = parameters
        .iter()
        .map(|(position, _, _)| *position as usize)
        .min()
        .ok_or("source telescope is empty")?;
    let parameter_identities = parameters
        .iter()
        .map(|(position, _, domain)| (*position, *domain))
        .collect::<Vec<_>>();
    if lifted.parameters.is_empty() {
        lifted.parameters = parameter_identities;
        lifted.base = first;
    } else if lifted.parameters != parameter_identities {
        return Err("source theorem changed its outer telescope");
    }
    lifted.copies.resize(original_len - first, None);
    let mut edges = vec![None; original_len - first];
    let mut pending = constant_references(arena, &[proof, ty])
        .ok_or("source theorem has a dummy term")?
        .into_iter()
        .collect::<Vec<_>>();
    while let Some(position) = pending.pop() {
        let position = position as usize;
        if position >= original_len {
            return Err("source theorem has an unformed declaration");
        }
        if position <= first
            || lifted.get(position as u32).is_some()
            || edges[position - first].is_some()
        {
            continue;
        }
        let declaration = &declarations[position];
        let roots = std::iter::once(declaration.ty)
            .chain(declaration.body)
            .collect::<Vec<_>>();
        let references = constant_references(arena, &roots)
            .ok_or("source theorem declaration has a dummy term")?;
        if references
            .iter()
            .any(|&reference| reference as usize >= position)
        {
            return Err("source theorem changed its declaration prefix");
        }
        pending.extend(references.iter().copied());
        edges[position - first] = Some(references);
    }
    for position in first + 1..original_len {
        let Some(references) = &edges[position - first] else {
            continue;
        };
        if !references.iter().any(|reference| {
            parameters
                .iter()
                .any(|(parameter, _, _)| parameter == reference)
                || lifted.get(*reference).is_some()
        }) {
            continue;
        }
        let declaration = declarations[position].clone();
        let Some(body) = declaration.body else {
            return Err("source theorem retains an unabstracted dependent assumption");
        };
        let (body, ty) =
            abstract_parameters_with_declarations(arena, body, declaration.ty, parameters, lifted)?;
        let replacement = declarations.len() as u32;
        declarations.push(Declaration::definition(declaration.level_arity, ty, body));
        lifted.copies[position - first] = Some(replacement);
    }
    let closed = abstract_parameters_with_declarations(arena, proof, ty, parameters, lifted)?;
    if !excludes_parameters(arena, declarations, &[closed.0, closed.1], parameters) {
        return Err("source theorem retains an outer parameter assumption after abstraction");
    }
    Ok(closed)
}

fn constant_references(arena: &TermArena, roots: &[TermHandle]) -> Option<Vec<u32>> {
    collect_constant_references(arena, roots, &mut Vec::new())
}

fn collect_constant_references(
    arena: &TermArena,
    roots: &[TermHandle],
    seen_terms: &mut Vec<u64>,
) -> Option<Vec<u32>> {
    let mut pending = roots.to_vec();
    let mut constants = Vec::new();
    while let Some(term) = pending.pop() {
        // A stale sibling must not hide behind the current slot's visited bit.
        let node = arena.get(term);
        if matches!(node, Term::Dummy) {
            return None;
        }
        let position = term.arena_index() as usize;
        let word = position / 64;
        if word >= seen_terms.len() {
            seen_terms.resize(word + 1, 0);
        }
        let bit = 1u64 << (position % 64);
        if seen_terms[word] & bit != 0 {
            continue;
        }
        seen_terms[word] |= bit;
        match node {
            Term::Constant { declaration, .. } => {
                constants.push(declaration);
            }
            Term::Pi { domain, codomain } | Term::Sigma { domain, codomain } => {
                pending.extend([domain, codomain])
            }
            Term::Lambda { domain, body } => pending.extend([domain, body]),
            Term::Apply { function, argument } => pending.extend([function, argument]),
            Term::Pair { first, second } => pending.extend([first, second]),
            Term::Fst { pair } | Term::Snd { pair } => pending.push(pair),
            Term::CaseTwo {
                motive,
                zero_branch,
                one_branch,
                scrutinee,
            } => pending.extend([motive, zero_branch, one_branch, scrutinee]),
            Term::Id { ty, left, right } => pending.extend([ty, left, right]),
            Term::Refl { ty, value }
            | Term::SquashIntro { ty, value }
            | Term::BoxIntro { ty, value } => pending.extend([ty, value]),
            Term::IdElim {
                motive,
                base,
                endpoint,
                proof,
            } => pending.extend([motive, base, endpoint, proof]),
            Term::W { carrier, children } => pending.extend([carrier, children]),
            Term::Sup {
                carrier,
                children,
                label,
                function,
            } => pending.extend([carrier, children, label, function]),
            Term::IndW { motive, step, tree } => pending.extend([motive, step, tree]),
            Term::EmptyElim { ty, scrutinee } => pending.extend([ty, scrutinee]),
            Term::Squash { ty } | Term::Box { ty } => pending.push(ty),
            Term::SquashElim {
                proposition,
                function,
                scrutinee,
            } => pending.extend([proposition, function, scrutinee]),
            Term::BoxElim {
                motive,
                body,
                scrutinee,
            } => pending.extend([motive, body, scrutinee]),
            Term::Dummy => return None,
            Term::Variable(_)
            | Term::Sort(_)
            | Term::Two
            | Term::TwoZero
            | Term::TwoOne
            | Term::Empty => {}
        }
    }
    Some(constants)
}

#[cfg(test)]
mod outer_type_closure_tests {
    use super::*;
    use proof_admission::{
        Budget, Context, Level, Sort, check_signature, check_type, judgment_assumption_closure,
    };

    #[test]
    fn a_stale_term_cannot_hide_behind_a_visited_slot() {
        let mut arena = TermArena::new();
        let current = arena.insert(Term::Two);
        let stale = TermHandle::from_parts(current.arena_index(), current.generation() + 1);
        // Stack order visits the current handle before its stale sibling.
        assert!(constant_references(&arena, &[stale, current]).is_none());
    }

    #[test]
    fn closes_shared_type_dependent_definitions_without_granting_an_assumption() {
        let mut arena = TermArena::new();
        let sort = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
        let parameter = SymbolHandle::from_arena_index(1);
        let mut declarations = vec![Declaration::assumption(0, sort)];
        let carrier = arena.insert(Term::Constant {
            declaration: 0,
            levels: vec![],
        });
        let value = arena.insert(Term::Variable(0));
        let relation = arena.insert(Term::Id {
            ty: carrier,
            left: value,
            right: value,
        });
        let ty = arena.insert(Term::Pi {
            domain: carrier,
            codomain: relation,
        });
        let reflexive = arena.insert(Term::Refl { ty: carrier, value });
        let body = arena.insert(Term::Lambda {
            domain: carrier,
            body: reflexive,
        });
        declarations.push(Declaration::definition(0, ty, body));
        let shared = arena.insert(Term::Constant {
            declaration: 1,
            levels: vec![],
        });
        declarations.push(Declaration::definition(0, ty, shared));
        let proof = arena.insert(Term::Constant {
            declaration: 2,
            levels: vec![],
        });
        check_signature(&mut arena, &declarations, &mut Budget::default()).unwrap();
        let parameters = [(0, parameter, sort)];
        let mut lifted = ParameterAbstractions::default();
        let (closed, statement) = abstract_parameter_closure(
            &mut arena,
            &mut declarations,
            proof,
            ty,
            &parameters,
            &mut lifted,
        )
        .unwrap();
        assert!(excludes_parameters(
            &arena,
            &declarations,
            &[closed, statement],
            &parameters
        ));
        let signature = check_signature(&mut arena, &declarations, &mut Budget::default()).unwrap();
        check_type(
            &mut arena,
            &Context::empty().with_signature(signature.clone()),
            closed,
            statement,
            &mut Budget::default(),
        )
        .unwrap();
        assert!(judgment_assumption_closure(&arena, &signature, &[closed, statement]).is_empty());
        let count = declarations.len();
        let _ = abstract_parameter_closure(
            &mut arena,
            &mut declarations,
            proof,
            ty,
            &parameters,
            &mut lifted,
        )
        .unwrap();
        assert_eq!(
            declarations.len(),
            count,
            "shared dependent definitions are lifted once"
        );

        // An arbitrary inhabitant cannot become a polymorphic theorem.
        let assumed = declarations.len() as u32;
        declarations.push(Declaration::assumption(0, carrier));
        let assumed = arena.insert(Term::Constant {
            declaration: assumed,
            levels: vec![],
        });
        assert!(
            abstract_parameter_closure(
                &mut arena,
                &mut declarations,
                assumed,
                carrier,
                &parameters,
                &mut lifted
            )
            .is_err()
        );
    }
}
