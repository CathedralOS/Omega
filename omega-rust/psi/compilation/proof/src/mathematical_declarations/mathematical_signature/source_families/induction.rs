//! Derived source-family induction. The invariant supplied here must be the
//! exact authored index proposition; source ownership and checked machine-body
//! authority remain the caller's obligations. This producer introduces no
//! proposition assumptions: case predicates and recursive hypotheses are bound.

use super::super::Elaborator;
use super::terms;
use diagnostics::Diagnostic;
use proof_admission::{
    Budget, Context, DEFAULT_CONVERSION_STEPS, Declaration, Level, Signature, Sort, Term,
    TermHandle,
};
use symbols::SymbolHandle;

/// An ordinary, kernel-checked theorem. This is not itself permission to
/// erase or cite a source machine.
#[derive(Clone)]
pub(in super::super) struct SourceInduction {
    pub theorem: TermHandle,
    pub ty: TermHandle,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_induction_reuse_retains_the_exact_signature_judgment() {
        let mut arena = proof_admission::TermArena::new();
        let two = arena.insert(Term::Two);
        let zero = arena.insert(Term::TwoZero);
        let one = arena.insert(Term::TwoOne);
        let b = arena.insert(Term::Constant {
            declaration: 0,
            levels: vec![],
        });
        let statement = arena.insert(Term::Id {
            ty: two,
            left: b,
            right: zero,
        });
        let proof = arena.insert(Term::Refl { ty: two, value: b });
        let declarations = vec![Declaration::definition(0, two, zero)];
        let check = |arena: &mut proof_admission::TermArena, declarations: &[Declaration], ty| {
            let signature = proof_admission::check_signature(
                arena,
                declarations,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
            )
            .unwrap();
            proof_admission::check_type(
                arena,
                &Context::empty().with_signature(signature),
                proof,
                ty,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
            )
        };
        check(&mut arena, &declarations, statement).unwrap();
        let cached = CheckedSourceInduction {
            raw_invariant: statement,
            result: SourceInduction {
                theorem: proof,
                ty: statement,
            },
            declarations: declarations.clone(),
        };
        let same_statement = arena.insert(Term::Id {
            ty: two,
            left: b,
            right: zero,
        });
        assert!(cached.reusable(&arena, &declarations, same_statement));
        let mut extended = declarations.clone();
        extended.push(Declaration::definition(0, two, one));
        assert!(cached.reusable(&arena, &extended, same_statement));
        check(&mut arena, &extended, statement).unwrap();

        // The raw handles stay identical, but changing b's transparent body
        // makes the formerly checked proof invalid in a still-valid signature.
        let changed = vec![Declaration::definition(0, two, one)];
        assert!(!cached.reusable(&arena, &changed, statement));
        assert!(check(&mut arena, &changed, statement).is_err());

        let changed_statement = arena.insert(Term::Id {
            ty: two,
            left: b,
            right: one,
        });
        assert!(!cached.reusable(&arena, &declarations, changed_statement));
        assert!(check(&mut arena, &declarations, changed_statement).is_err());
    }
}

/// Producer sharing within one immutable term arena and exact declaration
/// prefix. The final whole-signature check remains the admission boundary.
#[derive(Clone)]
pub(in super::super) struct CheckedSourceInduction {
    raw_invariant: TermHandle,
    result: SourceInduction,
    declarations: Vec<Declaration>,
}

impl CheckedSourceInduction {
    fn reusable(
        &self,
        arena: &proof_admission::TermArena,
        declarations: &[Declaration],
        raw_invariant: TermHandle,
    ) -> bool {
        declarations.starts_with(&self.declarations)
            && arena.structurally_equal(self.raw_invariant, raw_invariant)
    }
}

impl Elaborator<'_> {
    pub(in super::super) fn source_family_induction(
        &mut self,
        symbol: SymbolHandle,
        invariant: TermHandle,
    ) -> Result<SourceInduction, Vec<Diagnostic>> {
        let family = self.source_family(symbol)?;
        let raw_invariant = invariant;
        // The family model is immutable after its first construction. Raw
        // structural identity retains every constant and universe argument;
        // exact prefix equality additionally forbids reusing a derivation
        // after changing any referenced declaration's statement or body.
        // Appending declarations cannot change this checked judgment. Each
        // source occurrence still retains its own expression, carrier and
        // receiver-policy association at the caller.
        if let Some(previous) = family
            .checked_inductions
            .iter()
            .find(|previous| previous.reusable(&self.arena, &self.declarations, raw_invariant))
        {
            return Ok(previous.result.clone());
        }
        let sort = self
            .arena
            .insert(Term::Sort(Sort::Type(Level::Constant(0))));
        let invariant_type = self.arena.insert(Term::Pi {
            domain: family.description.index,
            codomain: sort,
        });
        self.check_induction_term(invariant, invariant_type, "index invariant")?;
        let invariant = terms::intern_invariant(
            &mut self.arena,
            &mut self.declarations,
            &family.description,
            invariant,
        );
        // A source specialization may erase every index occurrence from its
        // authored proposition. Prove that closed proposition directly;
        // generic index-dependent clauses still require genuine W induction.
        let signature = Signature::from_declarations(self.declarations.clone());
        let index = self.arena.insert(Term::Variable(0));
        let applied = self.arena.insert(Term::Apply {
            function: invariant,
            argument: index,
        });
        let goal = proof_admission::weak_head_normalize(
            &mut self.arena,
            &signature,
            applied,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .map_err(|error| {
            self.refuse(format!(
                "source invariant normalization is unsupported: {error:?}"
            ))
        })?;
        let context = Context::empty().with_signature(signature);
        if proof_admission::infer_sort(
            &mut self.arena,
            &context,
            goal,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .is_ok()
        {
            let proof = if let Some(theory) = self.integer_theory.as_mut() {
                theory
                    .closed_predicate(&mut self.arena, &mut self.declarations, goal)
                    .map_err(|error| {
                        self.refuse(format!(
                            "closed source invariant derivation is unsupported: {error:?}"
                        ))
                    })?
            } else {
                None
            };
            if let Some(proof) = proof {
                let (theorem, ty) = terms::constant_invariant_theorem(
                    &mut self.arena,
                    &family.description,
                    invariant,
                    proof,
                );
                self.check_induction_term(theorem, ty, "closed source invariant theorem")?;
                let result = SourceInduction { theorem, ty };
                self.record_source_induction(symbol, raw_invariant, &result);
                return Ok(result);
            }
        }
        let mut lemmas = Vec::new();
        let mut schemes = Vec::new();
        let common_types = family
            .common
            .iter()
            .map(|field| self.source_field_function(field, &family.parameters))
            .collect::<Result<Vec<_>, _>>()?;
        let mut common = family
            .common
            .iter()
            .zip(&common_types)
            .map(|(field, ty)| (field.symbol, *ty))
            .collect::<Vec<_>>();
        let data = self
            .program
            .unique_data_definition_by_symbol(symbol)
            .expect("retained source family declaration");
        let type_symbols = family
            .parameters
            .iter()
            .filter(|parameter| {
                matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
            })
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        let type_sort = self
            .arena
            .insert(Term::Sort(Sort::Type(Level::Constant(0))));
        common.splice(0..0, type_symbols.iter().map(|symbol| (*symbol, type_sort)));
        let mut index_fields = self
            .program
            .data_value_parameter_fields(data)
            .map(|(_, field)| field.symbol)
            .collect::<Vec<_>>();
        index_fields.splice(0..0, type_symbols.iter().copied());
        for case in &family.cases {
            let payload = case
                .payload
                .iter()
                .map(|field| {
                    self.source_field_function(field, &family.parameters)
                        .map(|ty| (field.symbol, ty))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let fields = common
                .iter()
                .chain(payload.iter())
                .copied()
                .collect::<Vec<_>>();
            let mut values = Vec::new();
            let mut parameters = Vec::new();
            for &(field, mut domain) in &fields {
                if !type_symbols.contains(&field) {
                    for actual in values.iter().take(type_symbols.len()) {
                        domain = self.arena.insert(Term::Apply {
                            function: domain,
                            argument: *actual,
                        });
                    }
                }
                // Temporary arbitrary values are not predicate axioms. Every
                // occurrence is abstracted before the theorem is checked.
                let position = self.declarations.len() as u32;
                self.declarations.push(Declaration {
                    level_arity: 0,
                    ty: domain,
                    body: None,
                });
                parameters.push((position, field, domain));
                values.push(self.constant(position));
            }
            let mut predicate = case.constraints;
            for &value in &values {
                predicate = self.arena.insert(Term::Apply {
                    function: predicate,
                    argument: value,
                });
            }
            let index_values = index_fields
                .iter()
                .map(|field| {
                    values[fields
                        .iter()
                        .position(|(candidate, _)| candidate == field)
                        .expect("exact source descriptor coordinate")]
                })
                .collect();
            let index = self.index_tuple(index_values)?;
            let goal = self.arena.insert(Term::Apply {
                function: invariant,
                argument: index,
            });
            let mut assumptions = vec![predicate];
            for &child in &case.child_indices {
                let mut index = child;
                for &value in &values {
                    index = self.arena.insert(Term::Apply {
                        function: index,
                        argument: value,
                    });
                }
                assumptions.push(self.arena.insert(Term::Apply {
                    function: invariant,
                    argument: index,
                }));
            }
            let Some(theory) = self.integer_theory.as_mut() else {
                return Err(
                    self.refuse("source induction requires a shared integer vocabulary".into())
                );
            };
            let proof = theory.predicate_from_assumptions(&mut self.arena, &mut self.declarations, goal, &assumptions)
                .map_err(|error| self.refuse(format!("source induction case arithmetic is unsupported: {error:?}")))?
                .ok_or_else(|| self.refuse("source induction case has no checked derivation from its exact constraints and child hypotheses".into()))?;
            let mut ty = goal;
            for &domain in assumptions.iter().rev() {
                ty = proof_admission::shift(&mut self.arena, ty, 0, 1);
                ty = self.arena.insert(Term::Pi {
                    domain,
                    codomain: ty,
                });
            }
            self.check_induction_term(proof, ty, "case predicate derivation")?;
            let (lemma, lemma_type) =
                terms::abstract_parameters(&mut self.arena, proof, ty, &parameters)
                    .map_err(|error| self.refuse(error.into()))?;
            if !terms::excludes_parameters(
                &self.arena,
                &self.declarations,
                &[lemma, lemma_type],
                &parameters,
            ) {
                return Err(self.refuse(
                    "source induction case retains an arbitrary value assumption after abstraction"
                        .into(),
                ));
            }
            self.check_induction_term(lemma, lemma_type, "abstracted source case")?;
            // Retain the checked case derivation once. The step applies its
            // typed declaration rather than repeatedly inferring its J body
            // at every occurrence inside the derived eliminator. Definitions
            // keep the complete proof/assumption closure reachable.
            let position = self.declarations.len() as u32;
            self.declarations
                .push(Declaration::definition(0, lemma_type, lemma));
            lemmas.push(self.constant(position));
            schemes.push(terms::Case {
                payload,
                constraints: case.constraints,
                child_indices: case.child_indices.clone(),
            });
        }
        let (theorem, ty, step, step_type) = terms::induction(
            &mut self.arena,
            &family.description,
            &common,
            &index_fields,
            &schemes,
            invariant,
            &lemmas,
            &mut self.declarations,
            &type_symbols,
        )
        .map_err(|error| self.refuse(error.into()))?;
        self.check_induction_term(step, step_type, "indexed-family induction step")?;
        self.check_induction_term(theorem, ty, "derived indexed-family theorem")?;
        // The final mathematical-signature trust boundary checks every
        // retained definition once, including this step and arithmetic laws.
        let result = SourceInduction { theorem, ty };
        self.record_source_induction(symbol, raw_invariant, &result);
        Ok(result)
    }

    fn record_source_induction(
        &mut self,
        symbol: SymbolHandle,
        raw_invariant: TermHandle,
        result: &SourceInduction,
    ) {
        self.indexed_families
            .get_mut(&symbol)
            .expect("retained source family")
            .checked_inductions
            .push(CheckedSourceInduction {
                raw_invariant,
                result: result.clone(),
                declarations: self.declarations.clone(),
            });
    }

    fn check_induction_term(
        &mut self,
        term: TermHandle,
        ty: TermHandle,
        role: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        let context = Context::empty()
            .with_signature(Signature::from_declarations(self.declarations.clone()));
        let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
        let result = proof_admission::check_type(&mut self.arena, &context, term, ty, &mut budget);
        result.map_err(|error| {
            self.refuse(format!("generated {role} fails kernel checking: {error:?}"))
        })
    }
}
