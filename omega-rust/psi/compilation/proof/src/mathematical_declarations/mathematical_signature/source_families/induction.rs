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
use typed_trees::data::TypeParameterKind;

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
                &Context::with_level_arity(
                    self.level_binders.len() as u32 + self.generalized_levels,
                )
                .with_signature(signature),
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
            parameter_actuals: Vec::new(),
        };
        let same_statement = arena.insert(Term::Id {
            ty: two,
            left: b,
            right: zero,
        });
        assert!(cached.reusable(&arena, &declarations, same_statement, &[]));
        let mut extended = declarations.clone();
        extended.push(Declaration::definition(0, two, one));
        assert!(cached.reusable(&arena, &extended, same_statement, &[]));
        check(&mut arena, &extended, statement).unwrap();

        // The raw handles stay identical, but changing b's transparent body
        // makes the formerly checked proof invalid in a still-valid signature.
        let changed = vec![Declaration::definition(0, two, one)];
        assert!(!cached.reusable(&arena, &changed, statement, &[]));
        assert!(check(&mut arena, &changed, statement).is_err());

        let changed_statement = arena.insert(Term::Id {
            ty: two,
            left: b,
            right: one,
        });
        assert!(!cached.reusable(&arena, &declarations, changed_statement, &[]));
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
    parameter_actuals: Vec<TermHandle>,
}

impl CheckedSourceInduction {
    fn reusable(
        &self,
        arena: &proof_admission::TermArena,
        declarations: &[Declaration],
        raw_invariant: TermHandle,
        parameter_actuals: &[TermHandle],
    ) -> bool {
        declarations.starts_with(&self.declarations)
            && self.parameter_actuals.len() == parameter_actuals.len()
            && self
                .parameter_actuals
                .iter()
                .zip(parameter_actuals)
                .all(|(&left, &right)| arena.structurally_equal(left, right))
            && arena.structurally_equal(self.raw_invariant, raw_invariant)
    }
}

fn term_mentions_variable(arena: &proof_admission::TermArena, term: TermHandle) -> bool {
    match arena.get(term) {
        Term::Variable(_) => true,
        Term::Constant { .. }
        | Term::Sort(_)
        | Term::Two
        | Term::TwoZero
        | Term::TwoOne
        | Term::Empty
        | Term::Dummy => false,
        Term::Apply { function, argument } => {
            term_mentions_variable(arena, function) || term_mentions_variable(arena, argument)
        }
        Term::Pair { first, second } => {
            term_mentions_variable(arena, first) || term_mentions_variable(arena, second)
        }
        Term::Lambda { domain, body }
        | Term::Pi {
            domain,
            codomain: body,
        } => term_mentions_variable(arena, domain) || term_mentions_variable(arena, body),
        _ => true,
    }
}

impl Elaborator<'_> {
    pub(in super::super) fn source_family_induction(
        &mut self,
        symbol: SymbolHandle,
        invariant: TermHandle,
        index_actuals: &[TermHandle],
    ) -> Result<SourceInduction, Vec<Diagnostic>> {
        let template = self.source_family(symbol)?;
        let family = self.applied_source_family(&template, index_actuals)?;
        let description = family
            .description_at(&mut self.arena, index_actuals)
            .map_err(|reason| self.refuse(reason.into()))?;
        let external_actuals = family
            .external_parameter_indices
            .iter()
            .map(|&position| (family.parameters[position].symbol, index_actuals[position]))
            .collect::<Vec<_>>();
        let parameter_actuals = external_actuals
            .iter()
            .map(|(_, actual)| *actual)
            .collect::<Vec<_>>();
        let all_parameter_actuals = index_actuals;
        let index_actuals = family.index_actuals(index_actuals);
        let raw_invariant = invariant;
        // The family model is immutable after its first construction. Raw
        // structural identity retains every constant and universe argument;
        // exact prefix equality additionally forbids reusing a derivation
        // after changing any referenced declaration's statement or body.
        // Appending declarations cannot change this checked judgment. Each
        // source occurrence still retains its own expression, carrier and
        // receiver-policy association at the caller.
        if let Some(previous) = family.checked_inductions.iter().find(|previous| {
            previous.reusable(
                &self.arena,
                &self.declarations,
                raw_invariant,
                &parameter_actuals,
            )
        }) {
            return Ok(previous.result.clone());
        }
        let result_level = terms::result_sort_level(
            &mut self.arena,
            &self.declarations,
            invariant,
            self.level_binders.len() as u32 + self.generalized_levels,
        )
        .map_err(|reason| self.refuse(reason.into()))?;
        let sort = self
            .arena
            .insert(Term::Sort(Sort::Type(result_level.clone())));
        let invariant_type = self.arena.insert(Term::Pi {
            domain: description.index,
            codomain: sort,
        });
        self.check_induction_term(invariant, invariant_type, "index invariant")?;
        let invariant = terms::intern_invariant(
            &mut self.arena,
            &mut self.declarations,
            &description,
            invariant,
            result_level,
        );
        // A source specialization may erase every index occurrence from its
        // authored proposition. When an open induction for this family is
        // already checked, apply it to these index actuals so the specialized
        // theorem cites the same laws. A closed proposition with no such
        // induction is proved directly. Generic index-dependent clauses still
        // require genuine W induction.
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
        let context =
            Context::with_level_arity(self.level_binders.len() as u32 + self.generalized_levels)
                .with_signature(signature);
        if proof_admission::infer_sort(
            &mut self.arena,
            &context,
            goal,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .is_ok()
        {
            // A closed specialization is this family's inductive theorem at
            // those indices. Instantiating it keeps the distributive law the
            // induction used. A fresh numeral equation would name a different
            // bodyless assumption and drop that law.
            if let Some(result) =
                self.instantiated_open_induction(&family, &index_actuals, &parameter_actuals)?
            {
                self.record_source_induction(symbol, raw_invariant, &result, &parameter_actuals);
                return Ok(result);
            }
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
                    &description,
                    invariant,
                    proof,
                );
                self.check_induction_term(theorem, ty, "closed source invariant theorem")?;
                let result = SourceInduction { theorem, ty };
                self.record_source_induction(symbol, raw_invariant, &result, &parameter_actuals);
                return Ok(result);
            }
        }
        let mut lemmas = Vec::new();
        let mut schemes = Vec::new();
        let common_types = family
            .common
            .iter()
            .map(|field| self.source_field_function(field, &family.parameters, &family.type_levels))
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
            .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        let mut external_slots = vec![false; family.parameters.len()];
        for &position in &family.external_parameter_indices {
            external_slots[position] = true;
        }
        let stored_type_slots = family
            .parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| matches!(parameter.kind, TypeParameterKind::Type))
            .enumerate()
            .filter_map(|(type_slot, (position, _))| {
                (!external_slots[position]).then_some(type_slot)
            })
            .collect::<Vec<_>>();
        let stored_type_symbols = stored_type_slots
            .iter()
            .map(|&slot| type_symbols[slot])
            .collect::<Vec<_>>();
        common.splice(
            0..0,
            stored_type_symbols.iter().map(|symbol| {
                let ordinal = type_symbols
                    .iter()
                    .position(|bound| bound == symbol)
                    .expect("retained Type binder");
                (
                    *symbol,
                    self.arena
                        .insert(Term::Sort(Sort::Type(family.type_levels[ordinal].clone()))),
                )
            }),
        );
        let mut index_fields = self
            .program
            .data_value_parameter_fields(data)
            .map(|(_, field)| field.symbol)
            .collect::<Vec<_>>();
        index_fields.splice(0..0, stored_type_symbols.iter().copied());
        for case in &family.cases {
            let payload = case
                .payload
                .iter()
                .map(|field| {
                    self.source_field_function(field, &family.parameters, &family.type_levels)
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
            let mut type_actuals = family
                .parameters
                .iter()
                .enumerate()
                .filter_map(|(position, parameter)| {
                    matches!(parameter.kind, TypeParameterKind::Type)
                        .then_some(all_parameter_actuals[position])
                })
                .collect::<Vec<_>>();
            for (field_ordinal, &(field, mut domain)) in fields.iter().enumerate() {
                let type_slot = stored_type_slots.get(field_ordinal).copied();
                if type_slot.is_none() {
                    for actual in &type_actuals {
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
                    level_arity: proof_admission::universe_parameter_arity(&self.arena, &[domain]),
                    ty: domain,
                    body: None,
                });
                parameters.push((position, field, domain));
                let value = self.constant(position);
                if let Some(type_slot) = type_slot {
                    type_actuals[type_slot] = value;
                }
                values.push(value);
            }
            let mut predicate = case.constraints;
            for &value in type_actuals
                .iter()
                .chain(values.iter().skip(stored_type_symbols.len()))
            {
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
                for &value in type_actuals
                    .iter()
                    .chain(values.iter().skip(stored_type_symbols.len()))
                {
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
            let (lemma, lemma_type) = super::parameter_closure::abstract_parameters(
                &mut self.arena,
                proof,
                ty,
                &parameters,
            )
            .map_err(|error| self.refuse(error.into()))?;
            if !super::parameter_closure::excludes_parameters(
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
            self.declarations.push(Declaration::definition(
                proof_admission::universe_parameter_arity(&self.arena, &[lemma_type, lemma]),
                lemma_type,
                lemma,
            ));
            lemmas.push(self.constant(position));
            schemes.push(terms::Case {
                payload,
                constraints: case.constraints,
                child_indices: case.child_indices.clone(),
            });
        }
        let (theorem, ty, step, step_type) = terms::induction(
            &mut self.arena,
            &description,
            &common,
            &index_fields,
            &schemes,
            invariant,
            &lemmas,
            &mut self.declarations,
            &type_symbols,
            &external_actuals,
            &family.payload_lifts,
        )
        .map_err(|error| self.refuse(error.into()))?;
        self.check_induction_term(step, step_type, "indexed-family induction step")?;
        self.check_induction_term(theorem, ty, "derived indexed-family theorem")?;
        // The final mathematical-signature trust boundary checks every
        // retained definition once, including this step and arithmetic laws.
        let result = SourceInduction { theorem, ty };
        self.record_source_induction(symbol, raw_invariant, &result, &parameter_actuals);
        Ok(result)
    }

    fn instantiated_open_induction(
        &mut self,
        family: &super::SourceFamily,
        index_actuals: &[TermHandle],
        parameter_actuals: &[TermHandle],
    ) -> Result<Option<SourceInduction>, Vec<Diagnostic>> {
        if index_actuals.is_empty() || index_actuals.len() != family.index_parameter_indices.len() {
            return Ok(None);
        }
        let signature = Signature::from_declarations(self.declarations.clone());
        let context =
            Context::with_level_arity(self.level_binders.len() as u32 + self.generalized_levels)
                .with_signature(signature.clone());
        if index_actuals
            .iter()
            .any(|actual| term_mentions_variable(&self.arena, *actual))
        {
            return Ok(None);
        }
        let open = {
            let Some(matched) = family.checked_inductions.iter().find(|previous| {
                if previous.parameter_actuals.len() != parameter_actuals.len()
                    || !previous
                        .parameter_actuals
                        .iter()
                        .zip(parameter_actuals)
                        .all(|(&left, &right)| self.arena.structurally_equal(left, right))
                {
                    return false;
                }
                let index = self.arena.insert(Term::Variable(0));
                let applied = self.arena.insert(Term::Apply {
                    function: previous.raw_invariant,
                    argument: index,
                });
                let Ok(goal) = proof_admission::weak_head_normalize(
                    &mut self.arena,
                    &signature,
                    applied,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                ) else {
                    return false;
                };
                proof_admission::infer_sort(
                    &mut self.arena,
                    &context,
                    goal,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .is_err()
            }) else {
                return Ok(None);
            };
            matched.result.clone()
        };
        let tuple = self.index_tuple(index_actuals.to_vec())?;
        let theorem = self.arena.insert(Term::Apply {
            function: open.theorem,
            argument: tuple,
        });
        let ty = proof_admission::infer_type(
            &mut self.arena,
            &context,
            theorem,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .map_err(|error| {
            self.refuse(format!(
                "instantiated source induction has no type: {error:?}"
            ))
        })?;
        self.check_induction_term(theorem, ty, "instantiated source induction")?;
        Ok(Some(SourceInduction { theorem, ty }))
    }

    fn record_source_induction(
        &mut self,
        symbol: SymbolHandle,
        raw_invariant: TermHandle,
        result: &SourceInduction,
        parameter_actuals: &[TermHandle],
    ) {
        self.indexed_families
            .get_mut(&symbol)
            .expect("retained source family")
            .checked_inductions
            .push(CheckedSourceInduction {
                raw_invariant,
                result: result.clone(),
                declarations: self.declarations.clone(),
                parameter_actuals: parameter_actuals.to_vec(),
            });
    }

    fn check_induction_term(
        &mut self,
        term: TermHandle,
        ty: TermHandle,
        role: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        let context =
            Context::with_level_arity(self.level_binders.len() as u32 + self.generalized_levels)
                .with_signature(Signature::from_declarations(self.declarations.clone()));
        let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
        let result = proof_admission::check_type(&mut self.arena, &context, term, ty, &mut budget);
        result.map_err(|error| {
            self.refuse(format!("generated {role} fails kernel checking: {error:?}"))
        })
    }
}
