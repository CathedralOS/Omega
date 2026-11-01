//! Denote exact proposition syntax without recursively owned logical keys.
//!
//! Syntax identities are invocation-local scratch, not semantic equivalence or
//! authority. Atomic keys keep their complete original spelling; logical keys
//! retain constructor and ordered child identities. Preparing these shallow
//! identities publishes no terms or declarations. Evaluation still enters each
//! uncached occurrence in the original order and publishes only on success.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SyntaxKey(usize);

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum LogicalSyntax {
    Conjunction(Vec<SyntaxKey>),
    Disjunction(Vec<SyntaxKey>),
    Implication {
        premise: SyntaxKey,
        conclusion: SyntaxKey,
    },
}

#[derive(Default)]
pub(in super::super) struct Cache {
    // These propositions contain no logical children. In particular, a cache
    // never clones, compares or drops an owned nested logical proposition.
    atoms: BTreeMap<Proposition, SyntaxKey>,
    logical: BTreeMap<LogicalSyntax, SyntaxKey>,
    // Append-only identities need no generation/recycling protocol. No key or
    // cache entry crosses the lifetime of this one denotation invocation.
    terms: Vec<Option<TermHandle>>,
}

impl Cache {
    pub(in super::super) fn cached_atom(&self, proposition: &Proposition) -> Option<TermHandle> {
        self.atoms
            .get(proposition)
            .and_then(|key| self.terms[key.0])
    }

    fn new_key(&mut self) -> SyntaxKey {
        let key = SyntaxKey(self.terms.len());
        self.terms.push(None);
        key
    }

    fn intern_atom(&mut self, proposition: &Proposition) -> SyntaxKey {
        if let Some(&key) = self.atoms.get(proposition) {
            return key;
        }
        let key = self.new_key();
        self.atoms.insert(proposition.clone(), key);
        key
    }

    fn intern_logical(&mut self, syntax: LogicalSyntax) -> SyntaxKey {
        if let Some(&key) = self.logical.get(&syntax) {
            return key;
        }
        let key = self.new_key();
        self.logical.insert(syntax, key);
        key
    }

    fn get(&self, key: Option<SyntaxKey>) -> Option<TermHandle> {
        key.and_then(|key| self.terms[key.0])
    }

    fn publish(&mut self, key: Option<SyntaxKey>, term: TermHandle) {
        let key = key.expect("an invalid logical arity cannot finish denotation");
        let slot = &mut self.terms[key.0];
        debug_assert!(slot.is_none() || *slot == Some(term));
        *slot = Some(term);
    }
}

struct Node<'source> {
    proposition: &'source Proposition,
    key: Option<SyntaxKey>,
    children: Vec<usize>,
}

struct Prepared<'source> {
    nodes: Vec<Node<'source>>,
    root: usize,
}

enum Prepare<'source> {
    Enter(&'source Proposition),
    Close(&'source Proposition, usize),
}

impl<'source> Prepared<'source> {
    fn new(cache: &mut Cache, proposition: &'source Proposition) -> Self {
        let mut nodes: Vec<Node<'source>> = Vec::new();
        let mut finished: Vec<usize> = Vec::new();
        let mut pending = vec![Prepare::Enter(proposition)];
        while let Some(work) = pending.pop() {
            let node = match work {
                Prepare::Enter(proposition) => {
                    match proposition {
                        Proposition::Conjunction(parts) | Proposition::Disjunction(parts) => {
                            if parts.len() >= 2 {
                                pending.push(Prepare::Close(proposition, parts.len()));
                                pending.extend(parts.iter().rev().map(Prepare::Enter));
                                continue;
                            }
                            // No successful cache entry can contain this node.
                            // Do not even inspect its members: evaluation keeps
                            // the original arity-before-members refusal point.
                            Node {
                                proposition,
                                key: None,
                                children: Vec::new(),
                            }
                        }
                        Proposition::Implication {
                            premise,
                            conclusion,
                        } => {
                            pending.push(Prepare::Close(proposition, 2));
                            pending.push(Prepare::Enter(conclusion));
                            pending.push(Prepare::Enter(premise));
                            continue;
                        }
                        _ => Node {
                            proposition,
                            key: Some(cache.intern_atom(proposition)),
                            children: Vec::new(),
                        },
                    }
                }
                Prepare::Close(proposition, count) => {
                    let children = finished.split_off(finished.len() - count);
                    let keys: Option<Vec<_>> =
                        children.iter().map(|&child| nodes[child].key).collect();
                    let key = keys.map(|keys| {
                        let syntax = match proposition {
                            Proposition::Conjunction(_) => LogicalSyntax::Conjunction(keys),
                            Proposition::Disjunction(_) => LogicalSyntax::Disjunction(keys),
                            Proposition::Implication { .. } => LogicalSyntax::Implication {
                                premise: keys[0],
                                conclusion: keys[1],
                            },
                            _ => unreachable!("only logical nodes have pending children"),
                        };
                        cache.intern_logical(syntax)
                    });
                    Node {
                        proposition,
                        key,
                        children,
                    }
                }
            };
            finished.push(nodes.len());
            nodes.push(node);
        }
        assert_eq!(finished.len(), 1);
        Self {
            nodes,
            root: finished[0],
        }
    }
}

struct Pending {
    node: usize,
    next: usize,
    terms: Vec<TermHandle>,
}

pub(super) fn denote(
    denotation: &mut Denotation,
    proposition: &Proposition,
) -> Result<TermHandle, BoundedDenotationError> {
    if let Some(term) = denotation.denotations.cached_atom(proposition) {
        return Ok(term);
    }
    let prepared = Prepared::new(&mut denotation.denotations, proposition);
    let mut pending: Vec<Pending> = Vec::new();
    let mut current = prepared.root;
    'enter: loop {
        let node = &prepared.nodes[current];
        let mut value = if let Some(term) = denotation.denotations.get(node.key) {
            term
        } else {
            match node.proposition {
                Proposition::Conjunction(parts) if parts.len() < 2 => {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::RuleConclusionMismatch("conjunction arity"),
                    ));
                }
                Proposition::Disjunction(parts) if parts.len() < 2 => {
                    return Err(BoundedDenotationError::Certificate(
                        ProofError::RuleConclusionMismatch("disjunction arity"),
                    ));
                }
                Proposition::Conjunction(_)
                | Proposition::Disjunction(_)
                | Proposition::Implication { .. } => {
                    let first = node.children[0];
                    pending.push(Pending {
                        node: current,
                        next: 1,
                        terms: Vec::with_capacity(node.children.len()),
                    });
                    current = first;
                    continue 'enter;
                }
                _ => {
                    let term = denotation.denote_leaf(node.proposition)?;
                    denotation.denotations.publish(node.key, term);
                    term
                }
            }
        };
        loop {
            let Some(mut frame) = pending.pop() else {
                return Ok(value);
            };
            frame.terms.push(value);
            let node = &prepared.nodes[frame.node];
            if frame.next < node.children.len() {
                current = node.children[frame.next];
                frame.next += 1;
                pending.push(frame);
                continue 'enter;
            }
            value = match node.proposition {
                Proposition::Conjunction(_) => denotation.conjunction_terms(frame.terms),
                Proposition::Disjunction(_) => denotation.disjunction_terms(frame.terms),
                Proposition::Implication { .. } => denotation.arena.insert(Term::Pi {
                    domain: frame.terms[0],
                    codomain: frame.terms[1],
                }),
                _ => unreachable!("only logical nodes have pending term children"),
            };
            denotation.denotations.publish(node.key, value);
        }
    }
}
