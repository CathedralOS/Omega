//! Typed conversion for the Π/Σ fragment, the `Two`/`Id`/`W` primitives
//! and the strict layer: β, pair-projection and constructor-scrutinee
//! `caseTwo`/`J`/`indW`/`unsq`/`unbox` weak-head normalization under a
//! step ceiling, pair eta at a `Sigma` shared type, typed function eta
//! at a `Pi` shared type, and definitional proof irrelevance gated on
//! the *shared type's* sort — never on the shape of either side.
//!
//! Function eta is the profile's selected extension (`inductive_profile.md`
//! §typed-function-eta): a lambda and a non-lambda convert only when the
//! shared type weak-head normalizes to `Pi`, and only through the typed
//! rule `x ↦ f x ≡ f`. The judgment never deletes a wrapper shape at a
//! non-function shared type and grants no pointwise-equality collapse:
//! `x ↦ g x` and `f` convert only when `g` and `f` already do.

use super::signature::Signature;
use super::substitution::{instantiate_levels, shift, substitute};
use super::term::{Level, Sort, Term, TermArena, TermHandle, levels_equal, sorts_equal};
use super::typing::{Context, CoreError, box_body_type, infer_sort, infer_type, w_step_type};

/// The default number of β steps a conversion attempt may take.
pub const DEFAULT_CONVERSION_STEPS: u32 = 65_536;

/// Bounded resource for normalization. Exhaustion is the typed error
/// `CoreError::StepCeiling`, never a false judgment and never a hang.
pub struct Budget {
    remaining_steps: u32,
}

impl Budget {
    pub fn new(remaining_steps: u32) -> Self {
        Self { remaining_steps }
    }

    pub fn remaining(&self) -> u32 {
        self.remaining_steps
    }

    fn consume(&mut self) -> Result<(), CoreError> {
        self.remaining_steps = self
            .remaining_steps
            .checked_sub(1)
            .ok_or(CoreError::StepCeiling)?;
        Ok(())
    }
}

impl Default for Budget {
    fn default() -> Self {
        Self::new(DEFAULT_CONVERSION_STEPS)
    }
}

/// β, pair-projection, constructor-scrutinee and constant-unfolding
/// weak-head normalization. Each reduction consumes one step.
///
/// Reduction is untyped — it takes the declaration `signature` it unfolds
/// constants against, never a context. A definition constant unfolds to
/// its instantiated body as a budgeted step (δ); an assumption constant
/// is a neutral atom and stays stuck. Unfolding terminates because
/// signatures are prefix-checked: every constant references a strictly
/// earlier declaration, so each step lowers the greatest reachable index.
pub fn weak_head_normalize(
    arena: &mut TermArena,
    signature: &Signature,
    term: TermHandle,
    budget: &mut Budget,
) -> Result<TermHandle, CoreError> {
    let mut current = term;
    loop {
        match arena.get(current) {
            Term::Constant {
                declaration,
                levels,
            } => {
                let Some(declaration) = signature.get(declaration) else {
                    return Err(CoreError::UnknownDeclaration {
                        declaration,
                        signature_len: signature.len(),
                    });
                };
                match declaration.body {
                    Some(body) => {
                        budget.consume()?;
                        current = instantiate_levels(arena, body, &levels).map_err(|index| {
                            CoreError::UnboundLevelParameter {
                                index,
                                arity: declaration.level_arity,
                            }
                        })?;
                    }
                    // An assumption constant is a neutral atom: it has no
                    // body to unfold and stays stuck.
                    None => return Ok(current),
                }
            }
            Term::Apply { function, argument } => {
                let head = weak_head_normalize(arena, signature, function, budget)?;
                match arena.get(head) {
                    Term::Lambda { body, .. } => {
                        budget.consume()?;
                        current = substitute(arena, body, argument);
                    }
                    _ => {
                        if head == function {
                            return Ok(current);
                        }
                        return Ok(arena.insert(Term::Apply {
                            function: head,
                            argument,
                        }));
                    }
                }
            }
            Term::Fst { pair } => {
                let head = weak_head_normalize(arena, signature, pair, budget)?;
                match arena.get(head) {
                    Term::Pair { first, .. } => {
                        budget.consume()?;
                        current = first;
                    }
                    _ => {
                        if head == pair {
                            return Ok(current);
                        }
                        return Ok(arena.insert(Term::Fst { pair: head }));
                    }
                }
            }
            Term::Snd { pair } => {
                let head = weak_head_normalize(arena, signature, pair, budget)?;
                match arena.get(head) {
                    Term::Pair { second, .. } => {
                        budget.consume()?;
                        current = second;
                    }
                    _ => {
                        if head == pair {
                            return Ok(current);
                        }
                        return Ok(arena.insert(Term::Snd { pair: head }));
                    }
                }
            }
            Term::CaseTwo {
                motive,
                zero_branch,
                one_branch,
                scrutinee,
            } => {
                // `caseTwo(C, d0, d1, zero) → d0` and `… one → d1`: a
                // constructor scrutinee selects its branch; a neutral
                // scrutinee keeps the elimination stuck. There is no
                // eta law for `Two` — a stuck `caseTwo` is its own
                // normal form.
                let head = weak_head_normalize(arena, signature, scrutinee, budget)?;
                match arena.get(head) {
                    Term::TwoZero => {
                        budget.consume()?;
                        current = zero_branch;
                    }
                    Term::TwoOne => {
                        budget.consume()?;
                        current = one_branch;
                    }
                    _ => {
                        if head == scrutinee {
                            return Ok(current);
                        }
                        return Ok(arena.insert(Term::CaseTwo {
                            motive,
                            zero_branch,
                            one_branch,
                            scrutinee: head,
                        }));
                    }
                }
            }
            Term::IdElim {
                motive,
                base,
                endpoint,
                proof,
            } => {
                // `J(C, d, y, refl A x) → d`: a reflexivity proof selects
                // the base case. Typing already equated the supplied
                // endpoint with the proof's recorded endpoint and the
                // `refl` value, so the match needs no endpoint re-check;
                // a neutral proof keeps the elimination stuck. There is
                // no identity eta — a stuck `J` is its own normal form.
                let head = weak_head_normalize(arena, signature, proof, budget)?;
                match arena.get(head) {
                    Term::Refl { .. } => {
                        budget.consume()?;
                        current = base;
                    }
                    _ => {
                        if head == proof {
                            return Ok(current);
                        }
                        return Ok(arena.insert(Term::IdElim {
                            motive,
                            base,
                            endpoint,
                            proof: head,
                        }));
                    }
                }
            }
            Term::IndW { motive, step, tree } => {
                // `indW(P, step, sup A B a k) → step a k (λ(b : B a).
                // indW(P, step, k b))`: a constructor tree supplies the
                // label, the child function, and the checked `B`
                // annotation the induction hypothesis's `B a` domain is
                // rebuilt from, so the reduct is exact even when the
                // surrounding `W` type is neutral. The hypothesis's
                // child function stays arbitrary — a neutral `k` never
                // blocks the step. A non-`sup` tree keeps the
                // elimination stuck; there is no W eta.
                let head = weak_head_normalize(arena, signature, tree, budget)?;
                match arena.get(head) {
                    Term::Sup {
                        children,
                        label,
                        function,
                        ..
                    } => {
                        budget.consume()?;
                        let bound_domain = arena.insert(Term::Apply {
                            function: children,
                            argument: label,
                        });
                        let shifted_motive = shift(arena, motive, 0, 1);
                        let shifted_step = shift(arena, step, 0, 1);
                        let shifted_function = shift(arena, function, 0, 1);
                        let bound = arena.insert(Term::Variable(0));
                        let child = arena.insert(Term::Apply {
                            function: shifted_function,
                            argument: bound,
                        });
                        let body = arena.insert(Term::IndW {
                            motive: shifted_motive,
                            step: shifted_step,
                            tree: child,
                        });
                        let hypothesis = arena.insert(Term::Lambda {
                            domain: bound_domain,
                            body,
                        });
                        let applied = arena.insert(Term::Apply {
                            function: step,
                            argument: label,
                        });
                        let applied = arena.insert(Term::Apply {
                            function: applied,
                            argument: function,
                        });
                        current = arena.insert(Term::Apply {
                            function: applied,
                            argument: hypothesis,
                        });
                    }
                    _ => {
                        if head == tree {
                            return Ok(current);
                        }
                        return Ok(arena.insert(Term::IndW {
                            motive,
                            step,
                            tree: head,
                        }));
                    }
                }
            }
            Term::EmptyElim { ty, scrutinee } => {
                // `sEmpty` has no constructors, so the eliminator never
                // fires — a well-typed scrutinee is always neutral. The
                // scrutinee still weak-head normalizes so the stuck
                // form is canonical.
                let head = weak_head_normalize(arena, signature, scrutinee, budget)?;
                if head == scrutinee {
                    return Ok(current);
                }
                return Ok(arena.insert(Term::EmptyElim {
                    ty,
                    scrutinee: head,
                }));
            }
            Term::SquashElim {
                proposition,
                function,
                scrutinee,
            } => {
                // `unsq P f (sq a) → f a`: a squashed witness hands the
                // unwrapped value to the function. A neutral scrutinee
                // keeps the elimination stuck; there is no squash eta.
                let head = weak_head_normalize(arena, signature, scrutinee, budget)?;
                match arena.get(head) {
                    Term::SquashIntro { value, .. } => {
                        budget.consume()?;
                        current = arena.insert(Term::Apply {
                            function,
                            argument: value,
                        });
                    }
                    _ => {
                        if head == scrutinee {
                            return Ok(current);
                        }
                        return Ok(arena.insert(Term::SquashElim {
                            proposition,
                            function,
                            scrutinee: head,
                        }));
                    }
                }
            }
            Term::BoxElim {
                motive,
                body,
                scrutinee,
            } => {
                // `unbox P f (box a) → f a`: a boxed proof opens at the
                // body's binder. A neutral scrutinee keeps the
                // elimination stuck; there is no box eta.
                let head = weak_head_normalize(arena, signature, scrutinee, budget)?;
                match arena.get(head) {
                    Term::BoxIntro { value, .. } => {
                        budget.consume()?;
                        current = arena.insert(Term::Apply {
                            function: body,
                            argument: value,
                        });
                    }
                    _ => {
                        if head == scrutinee {
                            return Ok(current);
                        }
                        return Ok(arena.insert(Term::BoxElim {
                            motive,
                            body,
                            scrutinee: head,
                        }));
                    }
                }
            }
            _ => return Ok(current),
        }
    }
}

/// Typed conversion: are `left` and `right` definitionally equal at
/// `shared_type` under `context`?
///
/// Precondition: the caller has already checked both sides at `shared_type`.
/// When the shared type's sort is `Strict(_)`, definitional proof irrelevance
/// makes every pair of inhabitants convertible and this returns `Ok(true)`
/// without inspecting the terms. That collapse is what makes the judgment
/// typed: it is decided by the sort of the shared type, never by the terms.
pub fn convertible(
    arena: &mut TermArena,
    context: &Context,
    left: TermHandle,
    right: TermHandle,
    shared_type: TermHandle,
    budget: &mut Budget,
) -> Result<bool, CoreError> {
    let shared_sort = infer_sort(arena, context, shared_type, budget)?;
    if shared_sort.is_strict() {
        return Ok(true);
    }

    // Reflexivity: a well-typed term converts to itself, and under de
    // Bruijn indices structural equality is alpha-equality, so the
    // typed traversal below can only answer `Ok(true)` for this pair.
    // Skipping it is what keeps shared subterms shared instead of
    // re-deciding each occurrence — a term like `IndexedAt i t`
    // appearing on both sides of a comparison does not re-unfold and
    // re-check its encoding. The probe stays behind the strict-sort
    // check so a malformed shared type still reports its own error
    // first.
    if arena.structurally_equal(left, right) {
        return Ok(true);
    }

    let left = weak_head_normalize(arena, context.signature(), left, budget)?;
    let right = weak_head_normalize(arena, context.signature(), right, budget)?;

    // Weak-head normalization can identify different syntax — a
    // projection of a literal pair, a selected `caseTwo` branch, an
    // unfolded definition — so the reflexivity probe runs once more on
    // the normalized heads before the typed structural recursion.
    if arena.structurally_equal(left, right) {
        return Ok(true);
    }

    match (arena.get(left), arena.get(right)) {
        (Term::Sort(left_sort), Term::Sort(right_sort)) => {
            // Universes convert when their levels convert semantically —
            // `Type max(u, v)` and `Type max(v, u)` are different syntax
            // for the same universe. The layers never mix: no
            // cumulativity, no `Type`-versus-`Strict` collapse.
            Ok(sorts_equal(&left_sort, &right_sort))
        }
        (Term::Two, Term::Two)
        | (Term::TwoZero, Term::TwoZero)
        | (Term::TwoOne, Term::TwoOne)
        | (Term::Empty, Term::Empty) => Ok(true),
        (
            Term::Pi {
                domain: left_domain,
                codomain: left_codomain,
            },
            Term::Pi {
                domain: right_domain,
                codomain: right_codomain,
            },
        ) => {
            let domain_sort = infer_sort(arena, context, left_domain, budget)?;
            let shared_domain = arena.insert(Term::Sort(domain_sort));
            if !convertible(
                arena,
                context,
                left_domain,
                right_domain,
                shared_domain,
                budget,
            )? {
                return Ok(false);
            }
            let extended = context.extend(left_domain);
            let codomain_sort = infer_sort(arena, &extended, left_codomain, budget)?;
            let shared_codomain = arena.insert(Term::Sort(codomain_sort));
            convertible(
                arena,
                &extended,
                left_codomain,
                right_codomain,
                shared_codomain,
                budget,
            )
        }
        (
            Term::Lambda {
                body: left_body, ..
            },
            Term::Lambda {
                body: right_body, ..
            },
        ) => {
            let type_head = weak_head_normalize(arena, context.signature(), shared_type, budget)?;
            match arena.get(type_head) {
                Term::Pi { domain, codomain } => {
                    let extended = context.extend(domain);
                    convertible(arena, &extended, left_body, right_body, codomain, budget)
                }
                _ => Ok(false),
            }
        }
        (
            Term::Sigma {
                domain: left_domain,
                codomain: left_codomain,
            },
            Term::Sigma {
                domain: right_domain,
                codomain: right_codomain,
            },
        ) => {
            let domain_sort = infer_sort(arena, context, left_domain, budget)?;
            let shared_domain = arena.insert(Term::Sort(domain_sort));
            if !convertible(
                arena,
                context,
                left_domain,
                right_domain,
                shared_domain,
                budget,
            )? {
                return Ok(false);
            }
            let extended = context.extend(left_domain);
            let codomain_sort = infer_sort(arena, &extended, left_codomain, budget)?;
            let shared_codomain = arena.insert(Term::Sort(codomain_sort));
            convertible(
                arena,
                &extended,
                left_codomain,
                right_codomain,
                shared_codomain,
                budget,
            )
        }
        (
            Term::Pair {
                first: left_first,
                second: left_second,
            },
            Term::Pair {
                first: right_first,
                second: right_second,
            },
        ) => {
            let type_head = weak_head_normalize(arena, context.signature(), shared_type, budget)?;
            match arena.get(type_head) {
                Term::Sigma { domain, codomain } => {
                    if !convertible(arena, context, left_first, right_first, domain, budget)? {
                        return Ok(false);
                    }
                    let second_type = substitute(arena, codomain, left_first);
                    convertible(
                        arena,
                        context,
                        left_second,
                        right_second,
                        second_type,
                        budget,
                    )
                }
                _ => Ok(false),
            }
        }
        (Term::Variable(left_index), Term::Variable(right_index)) => Ok(left_index == right_index),
        (
            Term::Constant {
                declaration: left_declaration,
                levels: left_levels,
            },
            Term::Constant {
                declaration: right_declaration,
                levels: right_levels,
            },
        ) => {
            // Stuck constants: both are assumptions — a definition would
            // have unfolded in weak-head normalization above. They
            // convert exactly when they name the same declaration at
            // semantically equal instantiations: `d(max(u, v))` is the
            // same assumption as `d(max(v, u))`. Both sides share
            // `shared_type` by the judgment, so a declaration match
            // already fixes the type; only the level arguments decide.
            Ok(left_declaration == right_declaration
                && left_levels.len() == right_levels.len()
                && left_levels
                    .iter()
                    .zip(right_levels.iter())
                    .all(|(left, right)| levels_equal(left, right)))
        }
        (
            Term::Apply {
                function: left_function,
                argument: left_argument,
            },
            Term::Apply {
                function: right_function,
                argument: right_argument,
            },
        ) => {
            // Neutral spines: the heads must convert and the arguments must
            // convert at the function's inferred Π domain.
            let function_type = infer_type(arena, context, left_function, budget)?;
            let type_head = weak_head_normalize(arena, context.signature(), function_type, budget)?;
            match arena.get(type_head) {
                Term::Pi { domain, .. } => {
                    if !convertible(
                        arena,
                        context,
                        left_argument,
                        right_argument,
                        domain,
                        budget,
                    )? {
                        return Ok(false);
                    }
                    convertible(
                        arena,
                        context,
                        left_function,
                        right_function,
                        function_type,
                        budget,
                    )
                }
                _ => Ok(false),
            }
        }
        (Term::Fst { pair: left_pair }, Term::Fst { pair: right_pair })
        | (Term::Snd { pair: left_pair }, Term::Snd { pair: right_pair }) => {
            // Neutral projections: the projected pairs must convert at
            // their inferred `Sigma`.
            let pair_type = infer_type(arena, context, left_pair, budget)?;
            let pair_head = weak_head_normalize(arena, context.signature(), pair_type, budget)?;
            match arena.get(pair_head) {
                Term::Sigma { .. } => {
                    convertible(arena, context, left_pair, right_pair, pair_head, budget)
                }
                _ => Ok(false),
            }
        }
        (
            Term::CaseTwo {
                motive: left_motive,
                zero_branch: left_zero_branch,
                one_branch: left_one_branch,
                scrutinee: left_scrutinee,
            },
            Term::CaseTwo {
                motive: right_motive,
                zero_branch: right_zero_branch,
                one_branch: right_one_branch,
                scrutinee: right_scrutinee,
            },
        ) => {
            // Two stuck eliminations compare componentwise: the motives
            // at the left motive's inferred `Π(_ : Two). Type w`, the
            // scrutinees at `Two`, and each branch at the left motive
            // applied to its constructor. A constructor scrutinee never
            // reaches here — weak-head normalization already selected
            // its branch — so only stuck scrutinees meet this rule.
            let motive_type = infer_type(arena, context, left_motive, budget)?;
            if !convertible(
                arena,
                context,
                left_motive,
                right_motive,
                motive_type,
                budget,
            )? {
                return Ok(false);
            }
            let two = arena.insert(Term::Two);
            if !convertible(arena, context, left_scrutinee, right_scrutinee, two, budget)? {
                return Ok(false);
            }
            let zero = arena.insert(Term::TwoZero);
            let zero_branch_type = arena.insert(Term::Apply {
                function: left_motive,
                argument: zero,
            });
            if !convertible(
                arena,
                context,
                left_zero_branch,
                right_zero_branch,
                zero_branch_type,
                budget,
            )? {
                return Ok(false);
            }
            let one = arena.insert(Term::TwoOne);
            let one_branch_type = arena.insert(Term::Apply {
                function: left_motive,
                argument: one,
            });
            convertible(
                arena,
                context,
                left_one_branch,
                right_one_branch,
                one_branch_type,
                budget,
            )
        }
        (
            Term::Id {
                ty: left_ty,
                left: left_left,
                right: left_right,
            },
            Term::Id {
                ty: right_ty,
                left: right_left,
                right: right_right,
            },
        ) => {
            // Two identity types compare componentwise: the carriers as
            // types at the left carrier's sort, then each endpoint at
            // the left carrier.
            let ty_sort = infer_sort(arena, context, left_ty, budget)?;
            let shared_ty = arena.insert(Term::Sort(ty_sort));
            if !convertible(arena, context, left_ty, right_ty, shared_ty, budget)? {
                return Ok(false);
            }
            if !convertible(arena, context, left_left, right_left, left_ty, budget)? {
                return Ok(false);
            }
            convertible(arena, context, left_right, right_right, left_ty, budget)
        }
        (
            Term::Refl {
                value: left_value, ..
            },
            Term::Refl {
                value: right_value, ..
            },
        ) => {
            // Two reflexivity proofs at a shared identity type: the `ty`
            // annotations each convert to the shared carrier, so only
            // the values decide — compared at the shared type's carrier.
            let type_head = weak_head_normalize(arena, context.signature(), shared_type, budget)?;
            match arena.get(type_head) {
                Term::Id { ty, .. } => {
                    convertible(arena, context, left_value, right_value, ty, budget)
                }
                _ => Ok(false),
            }
        }
        (
            Term::IdElim {
                motive: left_motive,
                base: left_base,
                endpoint: left_endpoint,
                proof: left_proof,
            },
            Term::IdElim {
                motive: right_motive,
                base: right_base,
                endpoint: right_endpoint,
                proof: right_proof,
            },
        ) => {
            // Two stuck eliminations compare componentwise: the motives
            // at the left motive's inferred `Π(y : A). Π(_ : Id A x y).
            // Type w`, the endpoints at the identity carrier `A`, the
            // proofs at the left proof's inferred identity type, and the
            // bases at `C x (refl A x)` for the fixed endpoint `x`. A
            // `refl` scrutinee never reaches here — weak-head
            // normalization already selected the base — so only stuck
            // proofs meet this rule.
            let motive_type = infer_type(arena, context, left_motive, budget)?;
            if !convertible(
                arena,
                context,
                left_motive,
                right_motive,
                motive_type,
                budget,
            )? {
                return Ok(false);
            }
            let proof_type = infer_type(arena, context, left_proof, budget)?;
            let proof_head = weak_head_normalize(arena, context.signature(), proof_type, budget)?;
            let (ty, fixed) = match arena.get(proof_head) {
                Term::Id { ty, left, .. } => (ty, left),
                _ => return Ok(false),
            };
            if !convertible(arena, context, left_endpoint, right_endpoint, ty, budget)? {
                return Ok(false);
            }
            if !convertible(arena, context, left_proof, right_proof, proof_head, budget)? {
                return Ok(false);
            }
            let refl = arena.insert(Term::Refl { ty, value: fixed });
            let at_fixed = arena.insert(Term::Apply {
                function: left_motive,
                argument: fixed,
            });
            let base_type = arena.insert(Term::Apply {
                function: at_fixed,
                argument: refl,
            });
            convertible(arena, context, left_base, right_base, base_type, budget)
        }
        (
            Term::W {
                carrier: left_carrier,
                children: left_children,
            },
            Term::W {
                carrier: right_carrier,
                children: right_children,
            },
        ) => {
            // Two W types compare componentwise: the carriers at their
            // common sort, then the branching families at the canonical
            // `Π(_ : A). Type v` rebuilt from the left carrier and the
            // left family's codomain level. A sort or level mismatch on
            // either side can never convert — without cumulativity the
            // two sides share no type to be compared at.
            let left_sort = infer_sort(arena, context, left_carrier, budget)?;
            let right_sort = infer_sort(arena, context, right_carrier, budget)?;
            if !sorts_equal(&left_sort, &right_sort) {
                return Ok(false);
            }
            let shared_carrier = arena.insert(Term::Sort(left_sort));
            if !convertible(
                arena,
                context,
                left_carrier,
                right_carrier,
                shared_carrier,
                budget,
            )? {
                return Ok(false);
            }
            let left_level = w_children_level(arena, context, left_children, budget)?;
            let right_level = w_children_level(arena, context, right_children, budget)?;
            let (Some(level), Some(other)) = (left_level, right_level) else {
                return Ok(false);
            };
            if !levels_equal(&level, &other) {
                return Ok(false);
            }
            let codomain = arena.insert(Term::Sort(Sort::Type(level)));
            let family_type = arena.insert(Term::Pi {
                domain: left_carrier,
                codomain,
            });
            convertible(
                arena,
                context,
                left_children,
                right_children,
                family_type,
                budget,
            )
        }
        (
            Term::Sup {
                label: left_label,
                function: left_function,
                ..
            },
            Term::Sup {
                label: right_label,
                function: right_function,
                ..
            },
        ) => {
            // Two constructor trees at a shared `W A B`: the labels
            // compare at `A`, then the child functions at `Π(b : B a).
            // W A B` built from the shared family and the left label —
            // the dependent second position mirrors pair conversion.
            // The annotation fields were already checked into the
            // shared `W`, so they decide nothing here; and there is no
            // W eta, so a `sup` never converts to a non-`sup`.
            let type_head = weak_head_normalize(arena, context.signature(), shared_type, budget)?;
            match arena.get(type_head) {
                Term::W { carrier, children } => {
                    if !convertible(arena, context, left_label, right_label, carrier, budget)? {
                        return Ok(false);
                    }
                    let domain = arena.insert(Term::Apply {
                        function: children,
                        argument: left_label,
                    });
                    let codomain = shift(arena, type_head, 0, 1);
                    let function_type = arena.insert(Term::Pi { domain, codomain });
                    convertible(
                        arena,
                        context,
                        left_function,
                        right_function,
                        function_type,
                        budget,
                    )
                }
                _ => Ok(false),
            }
        }
        (
            Term::IndW {
                motive: left_motive,
                step: left_step,
                tree: left_tree,
            },
            Term::IndW {
                motive: right_motive,
                step: right_step,
                tree: right_tree,
            },
        ) => {
            // Two stuck inductions compare componentwise: the motives
            // at the left motive's inferred `Π(_ : W A B). Type w`, the
            // trees at the left tree's inferred `W A B`, and the steps
            // at the induction step type rebuilt from them. A `sup`
            // tree never reaches here — weak-head normalization already
            // unfolded `step a k ih` — so only stuck trees meet this
            // rule.
            let motive_type = infer_type(arena, context, left_motive, budget)?;
            if !convertible(
                arena,
                context,
                left_motive,
                right_motive,
                motive_type,
                budget,
            )? {
                return Ok(false);
            }
            let tree_type = infer_type(arena, context, left_tree, budget)?;
            let tree_head = weak_head_normalize(arena, context.signature(), tree_type, budget)?;
            let (carrier, children) = match arena.get(tree_head) {
                Term::W { carrier, children } => (carrier, children),
                _ => return Ok(false),
            };
            if !convertible(arena, context, left_tree, right_tree, tree_head, budget)? {
                return Ok(false);
            }
            let step_type = w_step_type(arena, carrier, children, left_motive);
            convertible(arena, context, left_step, right_step, step_type, budget)
        }
        (Term::Squash { ty: left_ty }, Term::Squash { ty: right_ty })
        | (Term::Box { ty: left_ty }, Term::Box { ty: right_ty }) => {
            // Two squash or box types compare componentwise: the
            // payloads as types at the left payload's sort. For `Box`
            // that sort is `Strict v` — a universe whose own sort is
            // relevant — so two boxed propositions stay distinct; for
            // `Squash` it is `Type u`, a real comparison.
            let ty_sort = infer_sort(arena, context, left_ty, budget)?;
            let shared_ty = arena.insert(Term::Sort(ty_sort));
            convertible(arena, context, left_ty, right_ty, shared_ty, budget)
        }
        (
            Term::SquashIntro {
                value: left_value, ..
            },
            Term::SquashIntro {
                value: right_value, ..
            },
        ) => {
            // Two squashed values at a shared `Squash A`: the `ty`
            // annotations each convert to `A`, so only the values
            // decide — compared at the shared squash's payload. The
            // comparison is strict-collapsed before it runs; the arm
            // exists so the traversal stays honest if it ever meets
            // one at a non-collapsed shared type.
            let type_head = weak_head_normalize(arena, context.signature(), shared_type, budget)?;
            match arena.get(type_head) {
                Term::Squash { ty } => {
                    convertible(arena, context, left_value, right_value, ty, budget)
                }
                _ => Ok(false),
            }
        }
        (
            Term::BoxIntro {
                value: left_value, ..
            },
            Term::BoxIntro {
                value: right_value, ..
            },
        ) => {
            // Two boxed proofs at a shared `Box A`: only the values
            // decide, compared at `A` — where strictness collapses any
            // two well-typed proofs, so canonical boxes are always
            // equal. Neutrals never reach this arm, which is exactly
            // how boxing keeps irrelevance from escaping.
            let type_head = weak_head_normalize(arena, context.signature(), shared_type, budget)?;
            match arena.get(type_head) {
                Term::Box { ty } => {
                    convertible(arena, context, left_value, right_value, ty, budget)
                }
                _ => Ok(false),
            }
        }
        (
            Term::EmptyElim {
                ty: left_ty,
                scrutinee: left_scrutinee,
            },
            Term::EmptyElim {
                ty: right_ty,
                scrutinee: right_scrutinee,
            },
        ) => {
            // Two stuck ex falso eliminations compare componentwise:
            // the targets as types at the left target's sort, then the
            // scrutinees at `sEmpty` — where strict collapse already
            // equates them, so only the targets decide. No constructor
            // scrutinee ever reaches here: `sEmpty` has none.
            let ty_sort = infer_sort(arena, context, left_ty, budget)?;
            let shared_ty = arena.insert(Term::Sort(ty_sort));
            if !convertible(arena, context, left_ty, right_ty, shared_ty, budget)? {
                return Ok(false);
            }
            let empty = arena.insert(Term::Empty);
            convertible(
                arena,
                context,
                left_scrutinee,
                right_scrutinee,
                empty,
                budget,
            )
        }
        (
            Term::SquashElim {
                proposition: left_proposition,
                function: left_function,
                scrutinee: left_scrutinee,
            },
            Term::SquashElim {
                proposition: right_proposition,
                function: right_function,
                scrutinee: right_scrutinee,
            },
        ) => {
            // Two stuck squash eliminations compare componentwise: the
            // propositions at the left proposition's sort, the
            // scrutinees at the left scrutinee's inferred `Squash A`,
            // and the functions at `Π(_ : A). P` rebuilt from them. A
            // `sq` scrutinee never reaches here — weak-head
            // normalization already applied the function — and the
            // whole comparison sits under a strict shared type, so the
            // arm runs only on malformed inputs; it still decides
            // honestly.
            let proposition_sort = infer_sort(arena, context, left_proposition, budget)?;
            let shared_proposition = arena.insert(Term::Sort(proposition_sort));
            if !convertible(
                arena,
                context,
                left_proposition,
                right_proposition,
                shared_proposition,
                budget,
            )? {
                return Ok(false);
            }
            let scrutinee_type = infer_type(arena, context, left_scrutinee, budget)?;
            let scrutinee_head =
                weak_head_normalize(arena, context.signature(), scrutinee_type, budget)?;
            let carrier = match arena.get(scrutinee_head) {
                Term::Squash { ty } => ty,
                _ => return Ok(false),
            };
            if !convertible(
                arena,
                context,
                left_scrutinee,
                right_scrutinee,
                scrutinee_head,
                budget,
            )? {
                return Ok(false);
            }
            let codomain = shift(arena, left_proposition, 0, 1);
            let function_type = arena.insert(Term::Pi {
                domain: carrier,
                codomain,
            });
            convertible(
                arena,
                context,
                left_function,
                right_function,
                function_type,
                budget,
            )
        }
        (
            Term::BoxElim {
                motive: left_motive,
                body: left_body,
                scrutinee: left_scrutinee,
            },
            Term::BoxElim {
                motive: right_motive,
                body: right_body,
                scrutinee: right_scrutinee,
            },
        ) => {
            // Two stuck unboxings compare componentwise: the motives at
            // the left motive's inferred `Π(_ : Box A). s_v`, the
            // scrutinees at the left scrutinee's inferred `Box A`, and
            // the bodies at `Π(a : A). P (box a)` rebuilt from them. A
            // `box` scrutinee never reaches here — weak-head
            // normalization already applied the body.
            let motive_type = infer_type(arena, context, left_motive, budget)?;
            if !convertible(
                arena,
                context,
                left_motive,
                right_motive,
                motive_type,
                budget,
            )? {
                return Ok(false);
            }
            let scrutinee_type = infer_type(arena, context, left_scrutinee, budget)?;
            let scrutinee_head =
                weak_head_normalize(arena, context.signature(), scrutinee_type, budget)?;
            let payload = match arena.get(scrutinee_head) {
                Term::Box { ty } => ty,
                _ => return Ok(false),
            };
            if !convertible(
                arena,
                context,
                left_scrutinee,
                right_scrutinee,
                scrutinee_head,
                budget,
            )? {
                return Ok(false);
            }
            let body_type = box_body_type(arena, payload, left_motive);
            convertible(arena, context, left_body, right_body, body_type, budget)
        }
        (Term::Pair { first, second }, _) => {
            // Pair eta: a literal pair converts to a non-pair only when the
            // non-pair's projections convert to its components.
            let type_head = weak_head_normalize(arena, context.signature(), shared_type, budget)?;
            match arena.get(type_head) {
                Term::Sigma { domain, codomain } => {
                    let right_first = arena.insert(Term::Fst { pair: right });
                    if !convertible(arena, context, first, right_first, domain, budget)? {
                        return Ok(false);
                    }
                    let right_second = arena.insert(Term::Snd { pair: right });
                    let second_type = substitute(arena, codomain, right_first);
                    convertible(arena, context, second, right_second, second_type, budget)
                }
                _ => Ok(false),
            }
        }
        (_, Term::Pair { first, second }) => {
            let type_head = weak_head_normalize(arena, context.signature(), shared_type, budget)?;
            match arena.get(type_head) {
                Term::Sigma { domain, codomain } => {
                    let left_first = arena.insert(Term::Fst { pair: left });
                    if !convertible(arena, context, left_first, first, domain, budget)? {
                        return Ok(false);
                    }
                    let left_second = arena.insert(Term::Snd { pair: left });
                    let second_type = substitute(arena, codomain, left_first);
                    convertible(arena, context, left_second, second, second_type, budget)
                }
                _ => Ok(false),
            }
        }
        (Term::Lambda { .. }, _) | (_, Term::Lambda { .. }) => {
            // Function eta: `x ↦ f x ≡ f` at a checked `Pi`. Only the shared
            // type authorizes the rule — when it weak-head normalizes to
            // `Pi`, the context gains its domain, the fresh variable is
            // de Bruijn index 0 by construction, and the non-lambda side is
            // shifted under the new binder so none of its variables can
            // capture it. The comparison then runs at the exact codomain,
            // so an `f x` body converts only when `f` itself does; a
            // pointwise match is never enough. A strict `Pi` never reaches
            // here — irrelevance already collapsed its inhabitants.
            let type_head = weak_head_normalize(arena, context.signature(), shared_type, budget)?;
            match arena.get(type_head) {
                Term::Pi { domain, codomain } => {
                    let (lambda_body, other) = match (arena.get(left), arena.get(right)) {
                        (Term::Lambda { body, .. }, _) => (body, right),
                        (_, Term::Lambda { body, .. }) => (body, left),
                        _ => unreachable!("a lambda arm is present"),
                    };
                    let extended = context.extend(domain);
                    let lifted = shift(arena, other, 0, 1);
                    let fresh = arena.insert(Term::Variable(0));
                    let applied = arena.insert(Term::Apply {
                        function: lifted,
                        argument: fresh,
                    });
                    convertible(arena, &extended, lambda_body, applied, codomain, budget)
                }
                _ => Ok(false),
            }
        }
        _ => Ok(false),
    }
}

/// The `v` of a checked `Π(_ : A). Type v` branching family, or `None`
/// when the inferred type has another shape. W formation and `sup`
/// checking already enforce this shape, so `None` can only arise on a
/// malformed input — where `false` is the honest answer.
fn w_children_level(
    arena: &mut TermArena,
    context: &Context,
    family: TermHandle,
    budget: &mut Budget,
) -> Result<Option<Level>, CoreError> {
    let family_type = infer_type(arena, context, family, budget)?;
    let head = weak_head_normalize(arena, context.signature(), family_type, budget)?;
    let codomain = match arena.get(head) {
        Term::Pi { codomain, .. } => codomain,
        _ => return Ok(None),
    };
    let codomain_head = weak_head_normalize(arena, context.signature(), codomain, budget)?;
    match arena.get(codomain_head) {
        Term::Sort(Sort::Type(level)) => Ok(Some(level)),
        _ => Ok(None),
    }
}
