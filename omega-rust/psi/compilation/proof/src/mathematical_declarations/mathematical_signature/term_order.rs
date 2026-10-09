//! Deterministic ordering of elaborated kernel terms for symmetric relations
//! and conjunction normalization. Ordering never grants source value identity.

use super::Elaborator;
use proof_admission::{Level, Term, TermHandle};

impl Elaborator<'_> {
    pub(super) fn term_order_key(&self, term: TermHandle) -> Vec<u8> {
        let mut key = Vec::new();
        self.write_term_order_key(term, &mut key);
        key
    }

    pub(super) fn write_term_order_key(&self, term: TermHandle, out: &mut Vec<u8>) {
        match self.arena.get(term) {
            Term::Dummy => out.push(0),
            Term::Variable(index) => {
                out.push(1);
                out.extend_from_slice(&index.to_le_bytes());
            }
            Term::Sort(sort) => {
                out.push(2);
                out.push(sort.is_strict() as u8);
                self.write_level_key(sort.level(), out);
            }
            Term::Constant {
                declaration,
                levels,
            } => {
                out.push(3);
                out.extend_from_slice(&declaration.to_le_bytes());
                for level in levels {
                    self.write_level_key(level, out);
                }
            }
            Term::Pi { domain, codomain } => self.write_pair_key(4, domain, codomain, out),
            Term::Lambda { domain, body } => self.write_pair_key(5, domain, body, out),
            Term::Apply { function, argument } => self.write_pair_key(6, function, argument, out),
            Term::Sigma { domain, codomain } => self.write_pair_key(7, domain, codomain, out),
            Term::Pair { first, second } => self.write_pair_key(8, first, second, out),
            Term::Fst { pair } => {
                out.push(9);
                self.write_term_order_key(pair, out);
            }
            Term::Snd { pair } => {
                out.push(10);
                self.write_term_order_key(pair, out);
            }
            Term::Two => out.push(11),
            Term::TwoZero => out.push(12),
            Term::TwoOne => out.push(13),
            Term::CaseTwo {
                motive,
                zero_branch,
                one_branch,
                scrutinee,
            } => {
                out.push(14);
                self.write_term_order_key(motive, out);
                self.write_term_order_key(zero_branch, out);
                self.write_term_order_key(one_branch, out);
                self.write_term_order_key(scrutinee, out);
            }
            Term::Id { ty, left, right } => {
                out.push(15);
                self.write_term_order_key(ty, out);
                self.write_term_order_key(left, out);
                self.write_term_order_key(right, out);
            }
            Term::Refl { ty, value } => self.write_pair_key(16, ty, value, out),
            Term::IdElim {
                motive,
                base,
                endpoint,
                proof,
            } => {
                out.push(17);
                self.write_term_order_key(motive, out);
                self.write_term_order_key(base, out);
                self.write_term_order_key(endpoint, out);
                self.write_term_order_key(proof, out);
            }
            Term::W { carrier, children } => self.write_pair_key(18, carrier, children, out),
            Term::Sup {
                carrier,
                children,
                label,
                function,
            } => {
                out.push(19);
                self.write_term_order_key(carrier, out);
                self.write_term_order_key(children, out);
                self.write_term_order_key(label, out);
                self.write_term_order_key(function, out);
            }
            Term::IndW { motive, step, tree } => {
                out.push(20);
                self.write_term_order_key(motive, out);
                self.write_term_order_key(step, out);
                self.write_term_order_key(tree, out);
            }
            Term::Empty => out.push(21),
            Term::EmptyElim { ty, scrutinee } => self.write_pair_key(22, ty, scrutinee, out),
            Term::Squash { ty } => {
                out.push(23);
                self.write_term_order_key(ty, out);
            }
            Term::SquashIntro { ty, value } => self.write_pair_key(24, ty, value, out),
            Term::SquashElim {
                proposition,
                function,
                scrutinee,
            } => {
                out.push(25);
                self.write_term_order_key(proposition, out);
                self.write_term_order_key(function, out);
                self.write_term_order_key(scrutinee, out);
            }
            Term::Box { ty } => {
                out.push(26);
                self.write_term_order_key(ty, out);
            }
            Term::BoxIntro { ty, value } => self.write_pair_key(27, ty, value, out),
            Term::BoxElim {
                motive,
                body,
                scrutinee,
            } => {
                out.push(28);
                self.write_term_order_key(motive, out);
                self.write_term_order_key(body, out);
                self.write_term_order_key(scrutinee, out);
            }
        }
    }

    pub(super) fn write_pair_key(
        &self,
        tag: u8,
        first: TermHandle,
        second: TermHandle,
        out: &mut Vec<u8>,
    ) {
        out.push(tag);
        self.write_term_order_key(first, out);
        self.write_term_order_key(second, out);
    }

    pub(super) fn write_level_key(&self, level: Level, out: &mut Vec<u8>) {
        match level {
            Level::Constant(value) => {
                out.push(0);
                out.extend_from_slice(&value.to_le_bytes());
            }
            Level::Parameter(index) => {
                out.push(1);
                out.extend_from_slice(&index.to_le_bytes());
            }
            Level::Successor(inner) => {
                out.push(2);
                self.write_level_key(*inner, out);
            }
            Level::Maximum(left, right) => {
                out.push(3);
                self.write_level_key(*left, out);
                self.write_level_key(*right, out);
            }
        }
    }
}
