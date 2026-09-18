//! Typing and checking judgments for the Π/Σ fragment with stratified
//! relevant/strict universes — predicative formation, explicit closed
//! levels, no cumulativity, no self-typing universe — plus the inductive
//! profile's two-element type, relevant identity type, and W-type with
//! their dependent eliminators.

use super::conversion::{Budget, convertible, weak_head_normalize};
use super::signature::Signature;
use super::substitution::{instantiate_levels, shift, substitute};
use super::term::{Level, Sort, Term, TermArena, TermHandle};

/// The full ambient judgment scope `Σ; Δ; Γ`: the signature `Σ` of
/// declarations every `Constant` resolves through, the level arity `Δ` —
/// the count of universe parameters every `Level::Parameter` must stay
/// under — and the types of the bound variables `Γ`, innermost last.
/// Each stored type is well-scoped for its own prefix; `lookup` shifts it
/// into the full context. Extending a binder never touches the signature
/// or the level arity: constants and level parameters are judgment scope,
/// not de Bruijn-bound.
#[derive(Clone, Debug, Default)]
pub struct Context {
    bindings: Vec<TermHandle>,
    level_arity: u32,
    signature: Signature,
}

impl Context {
    /// The closed context: no term bindings, no universe parameters and
    /// no declarations, so every level must be a closed constant and no
    /// `Constant` resolves.
    pub fn empty() -> Self {
        Self {
            bindings: Vec::new(),
            level_arity: 0,
            signature: Signature::new(),
        }
    }

    /// The context of a judgment parametric over `level_arity` universe
    /// parameters: `Level::Parameter(i)` is in scope exactly when
    /// `i < level_arity`. This is the substrate of a universe-polymorphic
    /// declaration — the judgment holds for every level instantiation.
    /// The signature starts empty; `with_signature` installs one.
    pub fn with_level_arity(level_arity: u32) -> Self {
        Self {
            bindings: Vec::new(),
            level_arity,
            signature: Signature::new(),
        }
    }

    /// The number of universe parameters in scope for this judgment.
    pub fn level_arity(&self) -> u32 {
        self.level_arity
    }

    /// The declaration signature this judgment resolves `Constant`
    /// references against — the ambient `Σ`, unchanged by binders.
    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    /// The same judgment scope under `signature`. The signature is
    /// ambient, not a binder, so the local bindings and level arity carry
    /// over unchanged.
    pub fn with_signature(&self, signature: Signature) -> Context {
        Context {
            bindings: self.bindings.clone(),
            level_arity: self.level_arity,
            signature,
        }
    }

    /// A new context with one more innermost binding of type `domain`.
    /// Term binders never touch the level arity: level parameters are not
    /// de Bruijn-bound and do not shift under binders.
    pub fn extend(&self, domain: TermHandle) -> Context {
        let mut bindings = self.bindings.clone();
        bindings.push(domain);
        Context {
            bindings,
            level_arity: self.level_arity,
            signature: self.signature.clone(),
        }
    }

    /// The type of the variable at de Bruijn `index`, shifted by `index + 1`
    /// so it is well-scoped in the full context.
    pub fn lookup(&self, arena: &mut TermArena, index: u32) -> Option<TermHandle> {
        let position = self
            .bindings
            .len()
            .checked_sub(usize::try_from(index).ok()?.checked_add(1)?)?;
        let stored = self.bindings[position];
        Some(shift(arena, stored, 0, index + 1))
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

/// Every refusal of the mathematical core is a typed error; resource
/// exhaustion (`StepCeiling`) is an error, never a false judgment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreError {
    DummyTerm,
    UnboundVariable {
        index: u32,
        context_depth: usize,
    },
    /// The largest representable constant level cannot be succeeded — a
    /// malformed universe, not a judgment failure.
    LevelOverflow,
    /// `Level::Parameter(index)` must name one of the judgment's
    /// `level_arity` universe parameters. An out-of-scope parameter is a
    /// malformed universe: the certificate claims a judgment under a level
    /// scope the level never references.
    UnboundLevelParameter {
        index: u32,
        arity: u32,
    },
    NotASort {
        term: TermHandle,
        actual_type: TermHandle,
    },
    NotAFunction {
        function: TermHandle,
        actual_type: TermHandle,
    },
    NotAPair {
        pair: TermHandle,
        actual_type: TermHandle,
    },
    /// A `caseTwo` motive must be a family into a relevant universe
    /// `Π(_ : Two). Type w`. A strict codomain is not an admitted
    /// elimination target: strict motives belong to the reference core's
    /// boxing rules, not this eliminator.
    StrictCaseMotiveCodomain {
        codomain: TermHandle,
    },
    /// A `caseTwo` motive whose codomain is not a universe at all leaves
    /// `C t` without a type to check branches or results against.
    CaseMotiveCodomainNotAUniverse {
        codomain: TermHandle,
    },
    /// `Id A x y` requires `A : Type u`: the profile's identity type is
    /// relevant data over a relevant carrier, and a strict domain is a
    /// proposition where proof-relevant distinction has no meaning.
    StrictIdentityDomain {
        domain: TermHandle,
    },
    /// The scrutinee of an identity elimination must inhabit an identity
    /// type `Id A x y`; anything else leaves `J` without endpoints or a
    /// fixed side to take `refl` on.
    NotAnIdentity {
        proof: TermHandle,
        actual_type: TermHandle,
    },
    /// A `J` motive must take both the endpoint and the identity proof:
    /// after the endpoint domain, its codomain must still be a `Π` whose
    /// domain is `Id A x y`.
    IdentityMotiveCodomainNotAFunction {
        codomain: TermHandle,
    },
    /// A `J` motive must be a family into a relevant universe
    /// `Π(y : A). Π(_ : Id A x y). Type w`. A strict codomain is not an
    /// admitted elimination target: strict motives belong to the
    /// reference core's boxing rules, not this eliminator.
    StrictIdentityMotiveCodomain {
        codomain: TermHandle,
    },
    /// A `J` motive whose codomain is not a universe at all leaves
    /// `C y p` without a type to check the base or the result against.
    IdentityMotiveCodomainNotAUniverse {
        codomain: TermHandle,
    },
    /// `W A B` requires `A : Type u`: over a strict proposition a
    /// well-founded tree has no relevant data to carry.
    StrictWCarrier {
        carrier: TermHandle,
    },
    /// A `W` branching family must land in a relevant universe
    /// `Π(_ : A). Type v`. A strict codomain makes child positions
    /// propositions, which the reference core's boxing rules own.
    StrictWChildrenCodomain {
        codomain: TermHandle,
    },
    /// A `W` branching family whose codomain is not a universe leaves
    /// `B a` without a type for child positions.
    WChildrenCodomainNotAUniverse {
        codomain: TermHandle,
    },
    /// An `indW` tree must inhabit a `W` type; anything else leaves
    /// the induction without a carrier or branching family.
    NotAW {
        tree: TermHandle,
        actual_type: TermHandle,
    },
    /// An `indW` motive must be a family into a relevant universe
    /// `Π(_ : W A B). Type w`. A strict codomain is not an admitted
    /// elimination target: strict motives belong to the reference
    /// core's boxing rules, not this eliminator.
    StrictInductionMotiveCodomain {
        codomain: TermHandle,
    },
    /// An `indW` motive whose codomain is not a universe at all leaves
    /// `P t` without a type to check the step or the result against.
    InductionMotiveCodomainNotAUniverse {
        codomain: TermHandle,
    },
    /// `Squash A` and `sq` require `A : Type u`: squashing is the map
    /// from relevant types into strict propositions. A strict domain is
    /// already a proposition — there is nothing left to forget.
    StrictSquashDomain {
        domain: TermHandle,
    },
    /// An `unsq` scrutinee must inhabit a squash type `Squash A`;
    /// anything else leaves the elimination without a carrier.
    NotASquash {
        scrutinee: TermHandle,
        actual_type: TermHandle,
    },
    /// `unsq P f x` requires `P : Strict v`: squashed existence may
    /// flow into a strict proposition, never back into relevant data.
    SquashTargetNotStrict {
        proposition: TermHandle,
    },
    /// `Box A` and `box` require `A : Strict v`: boxing wraps a strict
    /// proposition as a relevant type. A relevant domain was never
    /// squashed, and boxing it would only re-wrap live data.
    NonStrictBoxDomain {
        domain: TermHandle,
    },
    /// An `unbox` scrutinee must inhabit a boxed type `Box A`;
    /// anything else leaves the elimination without a proposition to
    /// open.
    NotABox {
        scrutinee: TermHandle,
        actual_type: TermHandle,
    },
    /// An `unbox` motive whose codomain is not a universe leaves
    /// `P x` without a type to check the body or the result against.
    /// The reference core admits motive codomains at either sort —
    /// `Ty_j` or `P_j` — so only a non-sort rejects.
    BoxMotiveCodomainNotAUniverse {
        codomain: TermHandle,
    },
    /// A `Constant` names a declaration position the ambient signature
    /// does not have — a forward reference, a self-reference (the
    /// signature only holds the checked prefix), or a reference into an
    /// empty signature. Malformed, never a valid judgment.
    UnknownDeclaration {
        declaration: u32,
        signature_len: usize,
    },
    /// A `Constant` instantiation must supply exactly the declaration's
    /// own level arity — the judgment claims the declaration at precisely
    /// those universe parameters.
    DeclarationArityMismatch {
        declaration: u32,
        expected: u32,
        supplied: usize,
    },
    ArgumentTypeMismatch {
        expected: TermHandle,
        actual: TermHandle,
    },
    TypeMismatch {
        expected: TermHandle,
        actual: TermHandle,
    },
    StepCeiling,
}

/// The sort at which `term` is a type: infer its type, weak-head normalize
/// it, and require a `Term::Sort`.
pub fn infer_sort(
    arena: &mut TermArena,
    context: &Context,
    term: TermHandle,
    budget: &mut Budget,
) -> Result<Sort, CoreError> {
    let inferred = infer_type(arena, context, term, budget)?;
    let head = weak_head_normalize(arena, context.signature(), inferred, budget)?;
    match arena.get(head) {
        Term::Sort(sort) => Ok(sort),
        _ => Err(CoreError::NotASort {
            term,
            actual_type: head,
        }),
    }
}

/// The type of `term` under `context`, computed without search.
pub fn infer_type(
    arena: &mut TermArena,
    context: &Context,
    term: TermHandle,
    budget: &mut Budget,
) -> Result<TermHandle, CoreError> {
    match arena.get(term) {
        Term::Dummy => Err(CoreError::DummyTerm),
        Term::Variable(index) => context
            .lookup(arena, index)
            .ok_or(CoreError::UnboundVariable {
                index,
                context_depth: context.len(),
            }),
        Term::Sort(sort) => {
            // `Type u : Type (u+1)` and `Strict v : Type (v+1)`. The sort's
            // level must be well-formed under the judgment's level arity —
            // every `Sort` node reachable from a checked judgment passes
            // through this rule, so an out-of-scope parameter anywhere in
            // the term, the claimed type or a context binding rejects.
            check_level(context.level_arity(), &sort.level())?;
            let level = sort.level().successor().ok_or(CoreError::LevelOverflow)?;
            Ok(arena.insert(Term::Sort(Sort::Type(level))))
        }
        Term::Pi { domain, codomain } => {
            let domain_sort = infer_sort(arena, context, domain, budget)?;
            let extended = context.extend(domain);
            let codomain_sort = infer_sort(arena, &extended, codomain, budget)?;
            let level = domain_sort.level().maximum(codomain_sort.level());
            // The codomain's sort selects the layer; the level is the maximum.
            let result_sort = match codomain_sort {
                Sort::Strict(_) => Sort::Strict(level),
                Sort::Type(_) => Sort::Type(level),
            };
            Ok(arena.insert(Term::Sort(result_sort)))
        }
        Term::Lambda { domain, body } => {
            infer_sort(arena, context, domain, budget)?;
            let extended = context.extend(domain);
            let body_type = infer_type(arena, &extended, body, budget)?;
            Ok(arena.insert(Term::Pi {
                domain,
                codomain: body_type,
            }))
        }
        Term::Apply { function, argument } => {
            let function_type = infer_type(arena, context, function, budget)?;
            let function_head =
                weak_head_normalize(arena, context.signature(), function_type, budget)?;
            match arena.get(function_head) {
                Term::Pi { domain, codomain } => {
                    if let Term::Pair { .. } = arena.get(argument) {
                        // A dependent pair checks componentwise against a
                        // `Sigma` domain; its non-dependent inference could
                        // never convert there.
                        check_type(arena, context, argument, domain, budget)?;
                    } else {
                        let argument_type = infer_type(arena, context, argument, budget)?;
                        let domain_sort = infer_sort(arena, context, domain, budget)?;
                        let shared_type = arena.insert(Term::Sort(domain_sort));
                        if !convertible(arena, context, argument_type, domain, shared_type, budget)?
                        {
                            return Err(CoreError::ArgumentTypeMismatch {
                                expected: domain,
                                actual: argument_type,
                            });
                        }
                    }
                    Ok(substitute(arena, codomain, argument))
                }
                _ => Err(CoreError::NotAFunction {
                    function,
                    actual_type: function_head,
                }),
            }
        }
        Term::Sigma { domain, codomain } => {
            let domain_sort = infer_sort(arena, context, domain, budget)?;
            let extended = context.extend(domain);
            let codomain_sort = infer_sort(arena, &extended, codomain, budget)?;
            let level = domain_sort.level().maximum(codomain_sort.level());
            // A pair type is a strict proposition only when both components
            // are; any relevant component carries data and keeps the whole
            // type relevant.
            let result_sort = match (domain_sort, codomain_sort) {
                (Sort::Strict(_), Sort::Strict(_)) => Sort::Strict(level),
                _ => Sort::Type(level),
            };
            Ok(arena.insert(Term::Sort(result_sort)))
        }
        Term::Pair { first, second } => {
            // Inference is non-dependent: nothing records how the codomain
            // should mention the first component. `check_type` admits a
            // dependent pair componentwise against an expected `Sigma`.
            let first_type = infer_type(arena, context, first, budget)?;
            let second_type = infer_type(arena, context, second, budget)?;
            let codomain = shift(arena, second_type, 0, 1);
            Ok(arena.insert(Term::Sigma {
                domain: first_type,
                codomain,
            }))
        }
        Term::Fst { pair } => {
            let pair_type = infer_type(arena, context, pair, budget)?;
            let head = weak_head_normalize(arena, context.signature(), pair_type, budget)?;
            match arena.get(head) {
                Term::Sigma { domain, .. } => Ok(domain),
                _ => Err(CoreError::NotAPair {
                    pair,
                    actual_type: head,
                }),
            }
        }
        Term::Snd { pair } => {
            let pair_type = infer_type(arena, context, pair, budget)?;
            let head = weak_head_normalize(arena, context.signature(), pair_type, budget)?;
            match arena.get(head) {
                Term::Sigma { codomain, .. } => {
                    // `snd p : B[fst p]` — the dependent result keeps the
                    // projected first component.
                    let projected = arena.insert(Term::Fst { pair });
                    Ok(substitute(arena, codomain, projected))
                }
                _ => Err(CoreError::NotAPair {
                    pair,
                    actual_type: head,
                }),
            }
        }
        Term::Two => Ok(arena.insert(Term::Sort(Sort::Type(Level::Constant(0))))),
        Term::TwoZero | Term::TwoOne => Ok(arena.insert(Term::Two)),
        Term::CaseTwo {
            motive,
            zero_branch,
            one_branch,
            scrutinee,
        } => {
            // `caseTwo(C, d0, d1, t) : C t` for `C : Π(_ : Two). Type w`,
            // `d0 : C zero`, `d1 : C one` and `t : Two`. The motive's
            // codomain must normalize to a relevant universe: the
            // profile's eliminators target `Type`, and a `Strict`
            // codomain is a strict target owned by the reference core's
            // boxing rules, not this eliminator. The level `w` is read
            // off the checked codomain — elimination is not confined to
            // the scrutinee's level.
            let motive_type = infer_type(arena, context, motive, budget)?;
            let motive_head = weak_head_normalize(arena, context.signature(), motive_type, budget)?;
            let codomain = match arena.get(motive_head) {
                Term::Pi { domain, codomain } => {
                    let two = arena.insert(Term::Two);
                    let domain_sort = infer_sort(arena, context, domain, budget)?;
                    let shared_domain = arena.insert(Term::Sort(domain_sort));
                    if !convertible(arena, context, domain, two, shared_domain, budget)? {
                        return Err(CoreError::TypeMismatch {
                            expected: two,
                            actual: domain,
                        });
                    }
                    codomain
                }
                _ => {
                    return Err(CoreError::NotAFunction {
                        function: motive,
                        actual_type: motive_head,
                    });
                }
            };
            let codomain_head = weak_head_normalize(arena, context.signature(), codomain, budget)?;
            match arena.get(codomain_head) {
                Term::Sort(Sort::Type(_)) => {}
                Term::Sort(Sort::Strict(_)) => {
                    return Err(CoreError::StrictCaseMotiveCodomain { codomain });
                }
                _ => {
                    return Err(CoreError::CaseMotiveCodomainNotAUniverse { codomain });
                }
            }
            let two = arena.insert(Term::Two);
            check_type(arena, context, scrutinee, two, budget)?;
            let zero = arena.insert(Term::TwoZero);
            let zero_type = arena.insert(Term::Apply {
                function: motive,
                argument: zero,
            });
            check_type(arena, context, zero_branch, zero_type, budget)?;
            let one = arena.insert(Term::TwoOne);
            let one_type = arena.insert(Term::Apply {
                function: motive,
                argument: one,
            });
            check_type(arena, context, one_branch, one_type, budget)?;
            Ok(arena.insert(Term::Apply {
                function: motive,
                argument: scrutinee,
            }))
        }
        Term::Id { ty, left, right } => {
            // `Id A x y : Type u` for `A : Type u` and `x, y : A`. The
            // carrier must be a relevant type: over a strict proposition
            // there is no proof-relevant distinction for `Id` to carry.
            let domain_sort = infer_sort(arena, context, ty, budget)?;
            if domain_sort.is_strict() {
                return Err(CoreError::StrictIdentityDomain { domain: ty });
            }
            check_type(arena, context, left, ty, budget)?;
            check_type(arena, context, right, ty, budget)?;
            Ok(arena.insert(Term::Sort(domain_sort)))
        }
        Term::Refl { ty, value } => {
            // `refl A x : Id A x x`. The `ty` annotation is checked, not
            // trusted: `x` must inhabit `A` — through `check_type`, so a
            // dependent-pair endpoint still checks componentwise — and
            // `A` must be a relevant type, exactly as `Id` formation
            // requires.
            let domain_sort = infer_sort(arena, context, ty, budget)?;
            if domain_sort.is_strict() {
                return Err(CoreError::StrictIdentityDomain { domain: ty });
            }
            check_type(arena, context, value, ty, budget)?;
            Ok(arena.insert(Term::Id {
                ty,
                left: value,
                right: value,
            }))
        }
        Term::IdElim {
            motive,
            base,
            endpoint,
            proof,
        } => {
            // `J(C, d, y, p) : C y p` for `p : Id A x y`,
            // `C : Π(y : A). Π(_ : Id A x y). Type w`, and
            // `d : C x (refl A x)`. The proof's inferred identity type
            // supplies the fixed carrier `A` and fixed left endpoint
            // `x`; the supplied `y` must be the proof's recorded right
            // endpoint, so an elimination never silently relocates its
            // target.
            let proof_type = infer_type(arena, context, proof, budget)?;
            let proof_head = weak_head_normalize(arena, context.signature(), proof_type, budget)?;
            let (ty, fixed, recorded) = match arena.get(proof_head) {
                Term::Id { ty, left, right } => (ty, left, right),
                _ => {
                    return Err(CoreError::NotAnIdentity {
                        proof,
                        actual_type: proof_head,
                    });
                }
            };
            check_type(arena, context, endpoint, ty, budget)?;
            if !convertible(arena, context, recorded, endpoint, ty, budget)? {
                return Err(CoreError::TypeMismatch {
                    expected: recorded,
                    actual: endpoint,
                });
            }
            let motive_type = infer_type(arena, context, motive, budget)?;
            let motive_head = weak_head_normalize(arena, context.signature(), motive_type, budget)?;
            let (domain, first_codomain) = match arena.get(motive_head) {
                Term::Pi { domain, codomain } => (domain, codomain),
                _ => {
                    return Err(CoreError::NotAFunction {
                        function: motive,
                        actual_type: motive_head,
                    });
                }
            };
            let domain_sort = infer_sort(arena, context, domain, budget)?;
            let shared_domain = arena.insert(Term::Sort(domain_sort));
            if !convertible(arena, context, domain, ty, shared_domain, budget)? {
                return Err(CoreError::TypeMismatch {
                    expected: ty,
                    actual: domain,
                });
            }
            // Under the endpoint binder the motive must still be a
            // function of the identity proof, at `Id A x y` with `A`
            // and `x` shifted under the binder and `y` naming it.
            let extended = context.extend(domain);
            let first_codomain_head =
                weak_head_normalize(arena, context.signature(), first_codomain, budget)?;
            let (proof_domain, motive_codomain) = match arena.get(first_codomain_head) {
                Term::Pi { domain, codomain } => (domain, codomain),
                _ => {
                    return Err(CoreError::IdentityMotiveCodomainNotAFunction {
                        codomain: first_codomain,
                    });
                }
            };
            let shifted_ty = shift(arena, ty, 0, 1);
            let shifted_fixed = shift(arena, fixed, 0, 1);
            let bound = arena.insert(Term::Variable(0));
            let expected_domain = arena.insert(Term::Id {
                ty: shifted_ty,
                left: shifted_fixed,
                right: bound,
            });
            let proof_domain_sort = infer_sort(arena, &extended, proof_domain, budget)?;
            let shared_proof_domain = arena.insert(Term::Sort(proof_domain_sort));
            if !convertible(
                arena,
                &extended,
                proof_domain,
                expected_domain,
                shared_proof_domain,
                budget,
            )? {
                return Err(CoreError::TypeMismatch {
                    expected: expected_domain,
                    actual: proof_domain,
                });
            }
            // The motive level `w` is read off the checked codomain —
            // elimination is not confined to the carrier's level — but
            // it must be a relevant universe: strict targets belong to
            // the reference core's boxing rules.
            let codomain_head =
                weak_head_normalize(arena, context.signature(), motive_codomain, budget)?;
            match arena.get(codomain_head) {
                Term::Sort(Sort::Type(_)) => {}
                Term::Sort(Sort::Strict(_)) => {
                    return Err(CoreError::StrictIdentityMotiveCodomain {
                        codomain: motive_codomain,
                    });
                }
                _ => {
                    return Err(CoreError::IdentityMotiveCodomainNotAUniverse {
                        codomain: motive_codomain,
                    });
                }
            }
            // The base case supplies the reflexive instance at the
            // fixed endpoint: `d : C x (refl A x)`.
            let refl = arena.insert(Term::Refl { ty, value: fixed });
            let at_fixed = arena.insert(Term::Apply {
                function: motive,
                argument: fixed,
            });
            let base_type = arena.insert(Term::Apply {
                function: at_fixed,
                argument: refl,
            });
            check_type(arena, context, base, base_type, budget)?;
            let at_endpoint = arena.insert(Term::Apply {
                function: motive,
                argument: endpoint,
            });
            Ok(arena.insert(Term::Apply {
                function: at_endpoint,
                argument: proof,
            }))
        }
        Term::W { carrier, children } => {
            // `W A B : Type max(u, v)` — one shared formation rule,
            // also run for every `sup` annotation.
            let level = check_w_formation(arena, context, carrier, children, budget)?;
            Ok(arena.insert(Term::Sort(Sort::Type(level))))
        }
        Term::Sup {
            carrier,
            children,
            label,
            function,
        } => {
            // `sup A B a k : W A B`. The annotation re-runs the
            // formation rule — checked, never trusted — then the label
            // and the child function check at their places in it:
            // `k : Π(b : B a). W A B` with the `W` type shifted under
            // the new `b` binder.
            check_w_formation(arena, context, carrier, children, budget)?;
            check_type(arena, context, label, carrier, budget)?;
            let domain = arena.insert(Term::Apply {
                function: children,
                argument: label,
            });
            let w_type = arena.insert(Term::W { carrier, children });
            let codomain = shift(arena, w_type, 0, 1);
            let function_type = arena.insert(Term::Pi { domain, codomain });
            check_type(arena, context, function, function_type, budget)?;
            Ok(w_type)
        }
        Term::IndW { motive, step, tree } => {
            // `indW(P, step, t) : P t` for `t : W A B`,
            // `P : Π(_ : W A B). Type w`, and the induction step at
            // `w_step_type`. The tree's inferred `W` type supplies the
            // carrier and branching family — an elimination can never
            // relocate to a different `W`. The motive's codomain must
            // normalize to a relevant universe: strict targets belong
            // to the reference core's boxing rules, and the level `w`
            // is read off the checked codomain rather than confined to
            // the tree's level.
            let tree_type = infer_type(arena, context, tree, budget)?;
            let tree_head = weak_head_normalize(arena, context.signature(), tree_type, budget)?;
            let (carrier, children) = match arena.get(tree_head) {
                Term::W { carrier, children } => (carrier, children),
                _ => {
                    return Err(CoreError::NotAW {
                        tree,
                        actual_type: tree_head,
                    });
                }
            };
            let motive_type = infer_type(arena, context, motive, budget)?;
            let motive_head = weak_head_normalize(arena, context.signature(), motive_type, budget)?;
            let codomain = match arena.get(motive_head) {
                Term::Pi { domain, codomain } => {
                    let domain_sort = infer_sort(arena, context, domain, budget)?;
                    let shared_domain = arena.insert(Term::Sort(domain_sort));
                    if !convertible(arena, context, domain, tree_head, shared_domain, budget)? {
                        return Err(CoreError::TypeMismatch {
                            expected: tree_head,
                            actual: domain,
                        });
                    }
                    codomain
                }
                _ => {
                    return Err(CoreError::NotAFunction {
                        function: motive,
                        actual_type: motive_head,
                    });
                }
            };
            let codomain_head = weak_head_normalize(arena, context.signature(), codomain, budget)?;
            match arena.get(codomain_head) {
                Term::Sort(Sort::Type(_)) => {}
                Term::Sort(Sort::Strict(_)) => {
                    return Err(CoreError::StrictInductionMotiveCodomain { codomain });
                }
                _ => {
                    return Err(CoreError::InductionMotiveCodomainNotAUniverse { codomain });
                }
            }
            let step_type = w_step_type(arena, carrier, children, motive);
            check_type(arena, context, step, step_type, budget)?;
            Ok(arena.insert(Term::Apply {
                function: motive,
                argument: tree,
            }))
        }
        Term::Empty => {
            // `sEmpty : Strict 0` — the strict empty proposition sits at
            // level 0 like `Two` sits at `Type 0`: it carries no data,
            // and its eliminator reaches into every universe.
            Ok(arena.insert(Term::Sort(Sort::Strict(Level::Constant(0)))))
        }
        Term::EmptyElim { ty, scrutinee } => {
            // `sEmpty_rect A e : A` for `e : sEmpty`. The target is
            // checked to be a type at either sort — the reference core
            // eliminates the empty proposition into `Ty` and `P`
            // alike, since no closed scrutinee exists for the result
            // to depend on.
            infer_sort(arena, context, ty, budget)?;
            let empty = arena.insert(Term::Empty);
            check_type(arena, context, scrutinee, empty, budget)?;
            Ok(ty)
        }
        Term::Squash { ty } => {
            // `Squash A : Strict u` for `A : Type u`. The carrier must
            // be a relevant type: a strict `A` is already a
            // proposition, and squash adds nothing.
            let domain_sort = infer_sort(arena, context, ty, budget)?;
            match domain_sort {
                Sort::Type(level) => Ok(arena.insert(Term::Sort(Sort::Strict(level)))),
                Sort::Strict(_) => Err(CoreError::StrictSquashDomain { domain: ty }),
            }
        }
        Term::SquashIntro { ty, value } => {
            // `sq_A x : Squash A` for `x : A`. The annotation re-runs
            // the formation requirement — `A` must be a relevant type —
            // and the value checks at `A` through `check_type`, so a
            // dependent-pair witness still checks componentwise.
            let domain_sort = infer_sort(arena, context, ty, budget)?;
            if domain_sort.is_strict() {
                return Err(CoreError::StrictSquashDomain { domain: ty });
            }
            check_type(arena, context, value, ty, budget)?;
            Ok(arena.insert(Term::Squash { ty }))
        }
        Term::SquashElim {
            proposition,
            function,
            scrutinee,
        } => {
            // `unsq P f x : P` for `x : Squash A`, `P : Strict v` and
            // `f : Π(_ : A). P`. The scrutinee's inferred squash type
            // supplies the carrier `A` — an elimination never relocates
            // to a different squash. The target must be a strict
            // proposition: squashed existence proves propositions, and
            // the reference core's dependent eliminator is derived
            // through irrelevance rather than primitive.
            let scrutinee_type = infer_type(arena, context, scrutinee, budget)?;
            let scrutinee_head =
                weak_head_normalize(arena, context.signature(), scrutinee_type, budget)?;
            let carrier = match arena.get(scrutinee_head) {
                Term::Squash { ty } => ty,
                _ => {
                    return Err(CoreError::NotASquash {
                        scrutinee,
                        actual_type: scrutinee_head,
                    });
                }
            };
            match infer_sort(arena, context, proposition, budget)? {
                Sort::Strict(_) => {}
                Sort::Type(_) => {
                    return Err(CoreError::SquashTargetNotStrict { proposition });
                }
            }
            let codomain = shift(arena, proposition, 0, 1);
            let function_type = arena.insert(Term::Pi {
                domain: carrier,
                codomain,
            });
            check_type(arena, context, function, function_type, budget)?;
            Ok(proposition)
        }
        Term::Box { ty } => {
            // `Box A : Type v` for `A : Strict v`. The payload must be a
            // strict proposition: boxing wraps a proof-irrelevant type
            // as relevant data, and a relevant `A` was never squashed.
            let domain_sort = infer_sort(arena, context, ty, budget)?;
            match domain_sort {
                Sort::Strict(level) => Ok(arena.insert(Term::Sort(Sort::Type(level)))),
                Sort::Type(_) => Err(CoreError::NonStrictBoxDomain { domain: ty }),
            }
        }
        Term::BoxIntro { ty, value } => {
            // `box_A x : Box A` for `x : A`. The annotation re-runs the
            // formation requirement — `A` must be a strict proposition —
            // and the value checks at `A` through `check_type`.
            let domain_sort = infer_sort(arena, context, ty, budget)?;
            if !domain_sort.is_strict() {
                return Err(CoreError::NonStrictBoxDomain { domain: ty });
            }
            check_type(arena, context, value, ty, budget)?;
            Ok(arena.insert(Term::Box { ty }))
        }
        Term::BoxElim {
            motive,
            body,
            scrutinee,
        } => {
            // `unbox P f x : P x` for `x : Box A`,
            // `P : Π(_ : Box A). s_v` at either sort — the reference
            // core's eliminator lands in `Ty_j` or `P_j` alike — and
            // `f : Π(a : A). P (box a)`. The scrutinee's inferred box
            // supplies the payload `A`; the motive's domain must
            // convert to exactly that `Box A`.
            let scrutinee_type = infer_type(arena, context, scrutinee, budget)?;
            let scrutinee_head =
                weak_head_normalize(arena, context.signature(), scrutinee_type, budget)?;
            let payload = match arena.get(scrutinee_head) {
                Term::Box { ty } => ty,
                _ => {
                    return Err(CoreError::NotABox {
                        scrutinee,
                        actual_type: scrutinee_head,
                    });
                }
            };
            let motive_type = infer_type(arena, context, motive, budget)?;
            let motive_head = weak_head_normalize(arena, context.signature(), motive_type, budget)?;
            let codomain = match arena.get(motive_head) {
                Term::Pi { domain, codomain } => {
                    let domain_sort = infer_sort(arena, context, domain, budget)?;
                    let shared_domain = arena.insert(Term::Sort(domain_sort));
                    if !convertible(
                        arena,
                        context,
                        domain,
                        scrutinee_head,
                        shared_domain,
                        budget,
                    )? {
                        return Err(CoreError::TypeMismatch {
                            expected: scrutinee_head,
                            actual: domain,
                        });
                    }
                    codomain
                }
                _ => {
                    return Err(CoreError::NotAFunction {
                        function: motive,
                        actual_type: motive_head,
                    });
                }
            };
            let codomain_head = weak_head_normalize(arena, context.signature(), codomain, budget)?;
            match arena.get(codomain_head) {
                Term::Sort(_) => {}
                _ => {
                    return Err(CoreError::BoxMotiveCodomainNotAUniverse { codomain });
                }
            }
            // The body sees `P` instantiated at the boxed canonical
            // inhabitant: `f : Π(a : A). P (box a)`.
            let body_type = box_body_type(arena, payload, motive);
            check_type(arena, context, body, body_type, budget)?;
            Ok(arena.insert(Term::Apply {
                function: motive,
                argument: scrutinee,
            }))
        }
        Term::Constant {
            declaration: index,
            levels,
        } => {
            // `d(ls) : instantiate(decl[d].ty, ls)` — the declaration's
            // statement instantiated at the supplied level arguments. The
            // reference is checked, never trusted: the declaration must
            // exist in the ambient signature (which for a declaration
            // under check holds only its prefix, so self- and forward
            // references never resolve), the instantiation must supply
            // exactly its level arity, and every level argument must be
            // in scope under this judgment's own arity.
            let signature = context.signature();
            let Some(declaration) = signature.get(index) else {
                return Err(CoreError::UnknownDeclaration {
                    declaration: index,
                    signature_len: signature.len(),
                });
            };
            if levels.len() != declaration.level_arity as usize {
                return Err(CoreError::DeclarationArityMismatch {
                    declaration: index,
                    expected: declaration.level_arity,
                    supplied: levels.len(),
                });
            }
            for level in &levels {
                check_level(context.level_arity(), level)?;
            }
            instantiate_levels(arena, declaration.ty, &levels).map_err(|index| {
                CoreError::UnboundLevelParameter {
                    index,
                    arity: declaration.level_arity,
                }
            })
        }
    }
}

/// The closed-level scope rule: a level is well-formed under `arity` when
/// every `Parameter(i)` names one of the judgment's `arity` universe
/// parameters. Constant, successor and maximum levels introduce no scope
/// of their own, so the check is a structural walk; out-of-scope
/// parameters are malformed universes, never valid judgments.
fn check_level(arity: u32, level: &Level) -> Result<(), CoreError> {
    match level {
        Level::Constant(_) => Ok(()),
        Level::Parameter(index) => {
            if *index < arity {
                Ok(())
            } else {
                Err(CoreError::UnboundLevelParameter {
                    index: *index,
                    arity,
                })
            }
        }
        Level::Successor(inner) => check_level(arity, inner),
        Level::Maximum(left, right) => {
            check_level(arity, left)?;
            check_level(arity, right)
        }
    }
}

/// The `max(u, v)` level of a checked `W carrier children` formation:
/// `carrier : Type u` relevant and `children` a `Π(_ : carrier). Type v`
/// family into a relevant universe. `sup` annotations run through this
/// same rule — checked, never trusted.
fn check_w_formation(
    arena: &mut TermArena,
    context: &Context,
    carrier: TermHandle,
    children: TermHandle,
    budget: &mut Budget,
) -> Result<Level, CoreError> {
    let carrier_sort = infer_sort(arena, context, carrier, budget)?;
    let carrier_level = match carrier_sort {
        Sort::Type(level) => level,
        Sort::Strict(_) => return Err(CoreError::StrictWCarrier { carrier }),
    };
    let children_type = infer_type(arena, context, children, budget)?;
    let children_head = weak_head_normalize(arena, context.signature(), children_type, budget)?;
    let (domain, codomain) = match arena.get(children_head) {
        Term::Pi { domain, codomain } => (domain, codomain),
        _ => {
            return Err(CoreError::NotAFunction {
                function: children,
                actual_type: children_head,
            });
        }
    };
    let domain_sort = infer_sort(arena, context, domain, budget)?;
    let shared_domain = arena.insert(Term::Sort(domain_sort));
    if !convertible(arena, context, domain, carrier, shared_domain, budget)? {
        return Err(CoreError::TypeMismatch {
            expected: carrier,
            actual: domain,
        });
    }
    let codomain_head = weak_head_normalize(arena, context.signature(), codomain, budget)?;
    let children_level = match arena.get(codomain_head) {
        Term::Sort(Sort::Type(level)) => level,
        Term::Sort(Sort::Strict(_)) => {
            return Err(CoreError::StrictWChildrenCodomain { codomain });
        }
        _ => {
            return Err(CoreError::WChildrenCodomainNotAUniverse { codomain });
        }
    };
    Ok(carrier_level.maximum(children_level))
}

/// The checked type of an `indW` induction step:
/// `Π(a : A). Π(k : Π(b : B a). W A B). Π(_ : Π(b : B a). motive (k b)).
/// motive (sup A B a k)` — an induction hypothesis for every child.
///
/// `carrier`, `children` and `motive` are terms in the ambient context;
/// the builder shifts each under the binders it introduces. Depth 1
/// lives under `a`; depth 2 under `a, k`, which is also where `k`'s own
/// `b` binder leaves the `W A B` codomain; depth 3 under `a, k, ih` for
/// the result, and under `a, k` plus the hypothesis's `b` for its
/// codomain `motive (k b)`.
pub(super) fn w_step_type(
    arena: &mut TermArena,
    carrier: TermHandle,
    children: TermHandle,
    motive: TermHandle,
) -> TermHandle {
    let children_at_one = shift(arena, children, 0, 1);
    let bound_a = arena.insert(Term::Variable(0));
    let children_domain = arena.insert(Term::Apply {
        function: children_at_one,
        argument: bound_a,
    });
    let carrier_at_two = shift(arena, carrier, 0, 2);
    let children_at_two = shift(arena, children, 0, 2);
    let w_at_two = arena.insert(Term::W {
        carrier: carrier_at_two,
        children: children_at_two,
    });
    let function_type = arena.insert(Term::Pi {
        domain: children_domain,
        codomain: w_at_two,
    });
    let bound_a = arena.insert(Term::Variable(1));
    let hypothesis_domain = arena.insert(Term::Apply {
        function: children_at_two,
        argument: bound_a,
    });
    let motive_at_three = shift(arena, motive, 0, 3);
    let bound_k = arena.insert(Term::Variable(1));
    let bound_b = arena.insert(Term::Variable(0));
    let child = arena.insert(Term::Apply {
        function: bound_k,
        argument: bound_b,
    });
    let hypothesis_codomain = arena.insert(Term::Apply {
        function: motive_at_three,
        argument: child,
    });
    let hypothesis = arena.insert(Term::Pi {
        domain: hypothesis_domain,
        codomain: hypothesis_codomain,
    });
    let carrier_at_three = shift(arena, carrier, 0, 3);
    let children_at_three = shift(arena, children, 0, 3);
    let bound_a = arena.insert(Term::Variable(2));
    let bound_k = arena.insert(Term::Variable(1));
    let sup = arena.insert(Term::Sup {
        carrier: carrier_at_three,
        children: children_at_three,
        label: bound_a,
        function: bound_k,
    });
    let result = arena.insert(Term::Apply {
        function: motive_at_three,
        argument: sup,
    });
    let inner = arena.insert(Term::Pi {
        domain: hypothesis,
        codomain: result,
    });
    let middle = arena.insert(Term::Pi {
        domain: function_type,
        codomain: inner,
    });
    arena.insert(Term::Pi {
        domain: carrier,
        codomain: middle,
    })
}

/// The checked type of an `unbox` body:
/// `Π(a : A). motive (box_A a)` — the motive instantiated at the boxed
/// canonical inhabitant of the bound payload.
///
/// `payload` and `motive` are terms in the ambient context; the builder
/// shifts each under the `a` binder it introduces.
pub(super) fn box_body_type(
    arena: &mut TermArena,
    payload: TermHandle,
    motive: TermHandle,
) -> TermHandle {
    let shifted_motive = shift(arena, motive, 0, 1);
    let shifted_payload = shift(arena, payload, 0, 1);
    let bound = arena.insert(Term::Variable(0));
    let boxed = arena.insert(Term::BoxIntro {
        ty: shifted_payload,
        value: bound,
    });
    let body_codomain = arena.insert(Term::Apply {
        function: shifted_motive,
        argument: boxed,
    });
    arena.insert(Term::Pi {
        domain: payload,
        codomain: body_codomain,
    })
}

/// Check `term` against `expected`. The two types are compared at
/// `Sort(s)` where `s` is `expected`'s own sort — a universe, whose own
/// sort is always `Type`, so strict collapse can never fire for the
/// *types* themselves even when `expected` is a strict proposition such
/// as `sEmpty` or a squash's payload.
pub fn check_type(
    arena: &mut TermArena,
    context: &Context,
    term: TermHandle,
    expected: TermHandle,
    budget: &mut Budget,
) -> Result<(), CoreError> {
    let expected_sort = infer_sort(arena, context, expected, budget)?;
    // A pair against a `Sigma` checks componentwise, so the second
    // component sees the dependent codomain instantiated by the first.
    if let Term::Pair { first, second } = arena.get(term) {
        let head = weak_head_normalize(arena, context.signature(), expected, budget)?;
        if let Term::Sigma { domain, codomain } = arena.get(head) {
            check_type(arena, context, first, domain, budget)?;
            let second_type = substitute(arena, codomain, first);
            return check_type(arena, context, second, second_type, budget);
        }
    }
    let shared_type = arena.insert(Term::Sort(expected_sort));
    let actual = infer_type(arena, context, term, budget)?;
    if convertible(arena, context, actual, expected, shared_type, budget)? {
        Ok(())
    } else {
        Err(CoreError::TypeMismatch { expected, actual })
    }
}
