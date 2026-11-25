//! Unified native inductive types (Lean 4 / Rocq model), including mutually
//! recursive inductive blocks.
//!
//! One kernel mechanism handles plain recursive data, payloaded algebraic data
//! types (including dependent payloads), value-indexed inductive families, and
//! mutually recursive inductive blocks. A block of `M >= 1` mutual inductives
//! with `K` total constructors at signature position `base` occupies `2*M + K`
//! entries:
//!   - `base + m` (`0 <= m < M`): inductive type former `I_m`
//!   - `base + M + c` (`0 <= c < K`): constructor `C_c` (tagging its `I_m`)
//!   - `base + M + K + m` (`0 <= m < M`): recursor `I_m.rec`
//!
//! `check_inductive_block` validates universe predicativity, uniform parameter
//! consistency, mutual strict positivity of every constructor field, and exact
//! match with each kernel-computed recursor type. `reduce_recursor` performs
//! 1-step iota-reduction during weak-head normalization.

use std::collections::HashSet;
use std::rc::Rc;

use super::conversion::{Budget, weak_head_normalize};
use super::signature::{Declaration, DeclarationKind, Signature, SignatureCheckError};
use super::substitution::{instantiate, instantiate_levels, shift};
use super::term::{Level, Sort, Term, TermArena, TermHandle, level_leq, levels_equal};
use super::typing::{Context, CoreError, convert_retaining, infer_sort};

#[derive(Clone, Debug)]
pub(crate) struct RecursiveFieldSpec {
    pub field_index: usize,
    /// Mutual inductive index `0 <= target_inductive < num_mutual` of `I_j`.
    pub target_inductive: usize,
    /// Binder domains `D_0, ..., D_{r-1}` for a recursive field
    /// `Π(x_0 : D_0)...Π(x_{r-1} : D_{r-1}). I_j params... idxs...`.
    /// Each `D_m` is in context `[params..., fields[0..field_index]..., x_0..x_{m-1}]`.
    pub arg_binders: Vec<TermHandle>,
    /// Index arguments `idx_0, ..., idx_{ni_j-1}` in context
    /// `[params..., fields[0..field_index]..., x_0..x_{r-1}]`.
    pub index_args: Vec<TermHandle>,
}

#[derive(Clone, Debug)]
pub(crate) struct RecursorRule {
    pub recursive_fields: Vec<RecursiveFieldSpec>,
}

#[derive(Clone, Debug)]
pub(crate) struct ConstructorInfo {
    pub inductive: u32,
    pub constructor_index: usize,
    pub num_params: usize,
    pub num_fields: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct RecursorInfo {
    pub inductive: u32,
    pub block_base: u32,
    pub num_mutual: usize,
    pub inductive_level_arity: u32,
    pub num_params: usize,
    pub num_indices: usize,
    pub num_minors: usize,
    pub rules: Rc<Vec<RecursorRule>>,
}

impl RecursorInfo {
    pub(crate) fn total_args(&self) -> usize {
        self.num_params + self.num_mutual + self.num_minors + self.num_indices + 1
    }
}

struct ParsedConstructor {
    target_inductive: usize,
    field_types: Vec<TermHandle>,
    recursive_fields: Vec<RecursiveFieldSpec>,
    output_indices: Vec<TermHandle>,
}

fn identity_levels(arity: u32) -> Vec<Level> {
    (0..arity).map(Level::Parameter).collect()
}

fn levels_match_identity(levels: &[Level], arity: u32) -> bool {
    levels.len() == arity as usize
        && levels
            .iter()
            .enumerate()
            .all(|(i, level)| levels_equal(level, &Level::Parameter(i as u32)))
}

fn contains_any_inductive(arena: &TermArena, root: TermHandle, base: u32, num_mutual: u32) -> bool {
    let end = base.saturating_add(num_mutual);
    let mut pending = vec![root];
    let mut seen = HashSet::new();
    while let Some(term) = pending.pop() {
        if !seen.insert(term) {
            continue;
        }
        let node = arena.node(term);
        if let Term::Constant { declaration, .. } = node {
            if *declaration >= base && *declaration < end {
                return true;
            }
        } else {
            pending.extend(node.subterms().iter().map(|(child, _)| child));
        }
    }
    false
}

fn unwind_app(arena: &TermArena, mut term: TermHandle) -> (TermHandle, Vec<TermHandle>) {
    let mut args = Vec::new();
    while let Term::Apply { function, argument } = arena.get(term) {
        args.push(argument);
        term = function;
    }
    args.reverse();
    (term, args)
}

#[allow(clippy::too_many_arguments)]
fn check_inductive_head(
    arena: &mut TermArena,
    signature: &Signature,
    term: TermHandle,
    base: u32,
    num_mutual: u32,
    expected_target: Option<usize>,
    index_counts: &[usize],
    level_arity: u32,
    num_params: usize,
    depth_after_params: usize,
    budget: &mut Budget,
    wrong_head_error: CoreError,
) -> Result<(usize, Vec<TermHandle>), CoreError> {
    let (head, args) = unwind_app(arena, term);
    let Term::Constant {
        declaration,
        levels,
    } = arena.get(head)
    else {
        return Err(wrong_head_error);
    };
    let end = base.saturating_add(num_mutual);
    if declaration < base || declaration >= end || !levels_match_identity(&levels, level_arity) {
        return Err(wrong_head_error);
    }
    let matched_m = (declaration - base) as usize;
    if let Some(expected) = expected_target
        && matched_m != expected
    {
        return Err(wrong_head_error);
    }
    let num_indices = index_counts[matched_m];
    if args.len() != num_params + num_indices {
        return Err(wrong_head_error);
    }
    for (p, &arg) in args[..num_params].iter().enumerate() {
        let expected_var = (depth_after_params + (num_params - 1 - p)) as u32;
        let arg_whnf = weak_head_normalize(arena, signature, arg, budget)?;
        if arena.get(arg_whnf) != Term::Variable(expected_var) {
            return Err(CoreError::InductiveParameterMismatch);
        }
    }
    let indices = args[num_params..].to_vec();
    for &idx in &indices {
        if contains_any_inductive(arena, idx, base, num_mutual) {
            return Err(CoreError::NonStrictlyPositiveOccurrence);
        }
    }
    Ok((matched_m, indices))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn check_inductive_block(
    arena: &mut TermArena,
    signature: &mut Signature,
    declarations: &[Declaration],
    position: usize,
    num_mutual: u32,
    num_params: u32,
    num_constructors: u32,
    budget: &mut Budget,
) -> Result<usize, SignatureCheckError> {
    let nm = num_mutual as usize;
    let nc = num_constructors as usize;
    let block_len = 2 * nm + nc;
    if nm == 0 || position + block_len > declarations.len() {
        return Err(SignatureCheckError {
            declaration: position,
            error: CoreError::InvalidInductiveDeclaration,
        });
    }
    let base = u32::try_from(position).map_err(|_| SignatureCheckError {
        declaration: position,
        error: CoreError::InvalidInductiveDeclaration,
    })?;
    let level_arity = declarations[position].level_arity;
    let np = num_params as usize;

    // 1. Validate all `M` inductive type formers `I_0 .. I_{M-1}` in the preceding signature.
    let mut param_types: Vec<TermHandle> = Vec::new();
    let mut all_index_types: Vec<Vec<TermHandle>> = Vec::with_capacity(nm);
    let mut inductive_levels: Vec<Level> = Vec::with_capacity(nm);

    for m in 0..nm {
        let ind_pos = position + m;
        let inductive_decl = &declarations[ind_pos];
        let expected_kind = DeclarationKind::Inductive {
            num_mutual,
            mutual_index: m as u32,
            num_params,
            num_constructors,
        };
        if inductive_decl.kind != expected_kind
            || inductive_decl.body.is_some()
            || inductive_decl.level_arity != level_arity
        {
            return Err(SignatureCheckError {
                declaration: ind_pos,
                error: CoreError::InvalidInductiveDeclaration,
            });
        }
        let (params_m, index_types_m, level_m) = (|| {
            let mut ctx = Context::with_level_arity(level_arity).with_signature(signature.clone());
            infer_sort(arena, &ctx, inductive_decl.ty, budget)?;
            let mut binders = Vec::new();
            let mut current = inductive_decl.ty;
            loop {
                let head = weak_head_normalize(arena, ctx.signature(), current, budget)?;
                match arena.get(head) {
                    Term::Pi { domain, codomain } => {
                        binders.push(domain);
                        ctx = ctx.extend(domain);
                        current = codomain;
                    }
                    Term::Sort(Sort::Type(level)) => {
                        if binders.len() < np {
                            return Err(CoreError::InductiveParameterMismatch);
                        }
                        let index_types = binders.split_off(np);
                        return Ok((binders, index_types, level));
                    }
                    _ => return Err(CoreError::InductiveSortNotRelevant),
                }
            }
        })()
        .map_err(|error| SignatureCheckError {
            declaration: ind_pos,
            error,
        })?;

        if m == 0 {
            param_types = params_m;
        } else {
            // Verify uniform parameters match `I_0`'s parameter telescope.
            (|| {
                let mut ctx =
                    Context::with_level_arity(level_arity).with_signature(signature.clone());
                for (&expected_param, &actual_param) in param_types.iter().zip(params_m.iter()) {
                    let param_sort = infer_sort(arena, &ctx, expected_param, budget)?;
                    let sort_term = arena.insert(Term::Sort(param_sort));
                    let same = convert_retaining(
                        arena,
                        &ctx,
                        actual_param,
                        expected_param,
                        sort_term,
                        budget,
                        &|_| false,
                    )?;
                    if !same {
                        return Err(CoreError::InductiveParameterMismatch);
                    }
                    ctx = ctx.extend(expected_param);
                }
                Ok::<_, CoreError>(())
            })()
            .map_err(|error| SignatureCheckError {
                declaration: ind_pos,
                error,
            })?;
        }
        all_index_types.push(index_types_m);
        inductive_levels.push(level_m);
    }

    let index_counts: Vec<usize> = all_index_types.iter().map(Vec::len).collect();

    // 2. Tentatively add all `M` inductives to `signature` so constructors can refer to all of them.
    for m in 0..nm {
        signature.push(declarations[position + m].clone());
    }

    // 3. Validate each constructor `C_0 .. C_{K-1}`.
    let mut parsed_ctors = Vec::with_capacity(nc);
    for c in 0..nc {
        let ctor_pos = position + nm + c;
        let ctor_decl = &declarations[ctor_pos];
        let DeclarationKind::Constructor {
            inductive,
            constructor_index,
        } = ctor_decl.kind
        else {
            return Err(SignatureCheckError {
                declaration: ctor_pos,
                error: CoreError::InvalidInductiveDeclaration,
            });
        };
        if constructor_index != c as u32
            || inductive < base
            || inductive >= base.saturating_add(num_mutual)
            || ctor_decl.body.is_some()
            || ctor_decl.level_arity != level_arity
        {
            return Err(SignatureCheckError {
                declaration: ctor_pos,
                error: CoreError::InvalidInductiveDeclaration,
            });
        }
        let target_m = (inductive - base) as usize;
        let target_level = &inductive_levels[target_m];

        let parsed = (|| {
            let mut ctx = Context::with_level_arity(level_arity).with_signature(signature.clone());
            infer_sort(arena, &ctx, ctor_decl.ty, budget)?;

            // Check uniform parameters match `I_0`'s parameter telescope.
            let mut current = ctor_decl.ty;
            for &expected_param in &param_types {
                let head = weak_head_normalize(arena, ctx.signature(), current, budget)?;
                let Term::Pi { domain, codomain } = arena.get(head) else {
                    return Err(CoreError::InductiveParameterMismatch);
                };
                let param_sort = infer_sort(arena, &ctx, expected_param, budget)?;
                let sort_term = arena.insert(Term::Sort(param_sort));
                let same = convert_retaining(
                    arena,
                    &ctx,
                    domain,
                    expected_param,
                    sort_term,
                    budget,
                    &|_| false,
                )?;
                if !same {
                    return Err(CoreError::InductiveParameterMismatch);
                }
                ctx = ctx.extend(expected_param);
                current = codomain;
            }

            // Peel constructor fields, checking predicativity and mutual strict positivity.
            let mut field_types = Vec::new();
            let mut recursive_fields = Vec::new();
            loop {
                let head = weak_head_normalize(arena, ctx.signature(), current, budget)?;
                let Term::Pi { domain, codomain } = arena.get(head) else {
                    let (_, output_indices) = check_inductive_head(
                        arena,
                        ctx.signature(),
                        head,
                        base,
                        num_mutual,
                        Some(target_m),
                        &index_counts,
                        level_arity,
                        np,
                        field_types.len(),
                        budget,
                        CoreError::InvalidConstructorReturnType,
                    )?;
                    return Ok(ParsedConstructor {
                        target_inductive: target_m,
                        field_types,
                        recursive_fields,
                        output_indices,
                    });
                };

                // Universe predicativity check: field sort level must be <= level(I_{target_m}).
                let field_sort = infer_sort(arena, &ctx, domain, budget)?;
                if !level_leq(&field_sort.level(), target_level) {
                    return Err(CoreError::InductiveUniverseViolation);
                }

                // Mutual strict positivity check.
                let field_index = field_types.len();
                if contains_any_inductive(arena, domain, base, num_mutual) {
                    let mut arg_binders = Vec::new();
                    let mut field_ctx = ctx.clone();
                    let mut field_cur = domain;
                    loop {
                        let fh =
                            weak_head_normalize(arena, field_ctx.signature(), field_cur, budget)?;
                        if let Term::Pi {
                            domain: arg_dom,
                            codomain: arg_cod,
                        } = arena.get(fh)
                        {
                            if contains_any_inductive(arena, arg_dom, base, num_mutual) {
                                return Err(CoreError::NonStrictlyPositiveOccurrence);
                            }
                            arg_binders.push(arg_dom);
                            field_ctx = field_ctx.extend(arg_dom);
                            field_cur = arg_cod;
                        } else {
                            let (rec_m, index_args) = check_inductive_head(
                                arena,
                                field_ctx.signature(),
                                fh,
                                base,
                                num_mutual,
                                None,
                                &index_counts,
                                level_arity,
                                np,
                                field_index + arg_binders.len(),
                                budget,
                                CoreError::NonStrictlyPositiveOccurrence,
                            )?;
                            if !level_leq(&inductive_levels[rec_m], target_level) {
                                return Err(CoreError::InductiveUniverseViolation);
                            }
                            recursive_fields.push(RecursiveFieldSpec {
                                field_index,
                                target_inductive: rec_m,
                                arg_binders,
                                index_args,
                            });
                            break;
                        }
                    }
                }

                field_types.push(domain);
                ctx = ctx.extend(domain);
                current = codomain;
            }
        })()
        .map_err(|error| SignatureCheckError {
            declaration: ctor_pos,
            error,
        })?;
        parsed_ctors.push(parsed);
    }

    // 4. Compute canonical recursor types and verify all `M` recursor declarations.
    for m in 0..nm {
        let rec_pos = position + nm + nc + m;
        let rec_decl = &declarations[rec_pos];
        if rec_decl.kind
            != (DeclarationKind::Recursor {
                inductive: base + m as u32,
            })
            || rec_decl.body.is_some()
            || rec_decl.level_arity != level_arity + 1
        {
            return Err(SignatureCheckError {
                declaration: rec_pos,
                error: CoreError::InvalidInductiveDeclaration,
            });
        }
        let expected_rec_ty = build_recursor_type(
            arena,
            base,
            m,
            level_arity,
            &param_types,
            &all_index_types,
            &parsed_ctors,
        );
        if !arena.structurally_equal(rec_decl.ty, expected_rec_ty) {
            return Err(SignatureCheckError {
                declaration: rec_pos,
                error: CoreError::InvalidRecursorType,
            });
        }
    }

    // 5. Register all `K` constructors and all `M` recursors into `signature`.
    let mut rules = Vec::with_capacity(nc);
    for (c, parsed) in parsed_ctors.into_iter().enumerate() {
        let ctor_idx = base + num_mutual + c as u32;
        signature.push(declarations[position + nm + c].clone());
        signature.register_constructor(
            ctor_idx,
            ConstructorInfo {
                inductive: base + parsed.target_inductive as u32,
                constructor_index: c,
                num_params: np,
                num_fields: parsed.field_types.len(),
            },
        );
        rules.push(RecursorRule {
            recursive_fields: parsed.recursive_fields,
        });
    }
    let shared_rules = Rc::new(rules);
    for (m, &ni) in index_counts.iter().enumerate() {
        let rec_idx = base + num_mutual + num_constructors + m as u32;
        signature.push(declarations[position + nm + nc + m].clone());
        signature.register_recursor(
            rec_idx,
            RecursorInfo {
                inductive: base + m as u32,
                block_base: base,
                num_mutual: nm,
                inductive_level_arity: level_arity,
                num_params: np,
                num_indices: ni,
                num_minors: nc,
                rules: Rc::clone(&shared_rules),
            },
        );
    }

    Ok(block_len)
}

fn build_recursor_type(
    arena: &mut TermArena,
    base: u32,
    target_m: usize,
    level_arity: u32,
    param_types: &[TermHandle],
    all_index_types: &[Vec<TermHandle>],
    ctors: &[ParsedConstructor],
) -> TermHandle {
    let nm = all_index_types.len();
    let np = param_types.len();
    let nc = ctors.len();
    let id_levels = identity_levels(level_arity);
    let motive_sort = arena.insert(Term::Sort(Sort::Type(Level::Parameter(level_arity))));

    // Build all `M` motive types `P_0 .. P_{M-1}`.
    // `P_m` sits under `[params..., P_0..P_{m-1}]`, so `m` extra binders sit between `params` and `P_m`'s indices.
    let mut motive_tys = Vec::with_capacity(nm);
    for (m, index_types_m) in all_index_types.iter().enumerate() {
        let ni_m = index_types_m.len();
        let ind_const = arena.insert(Term::Constant {
            declaration: base + m as u32,
            levels: id_levels.clone(),
        });
        let mut major_ty = ind_const;
        for a in 0..np {
            let p_var = arena.insert(Term::Variable((ni_m + m + (np - 1 - a)) as u32));
            major_ty = arena.insert(Term::Apply {
                function: major_ty,
                argument: p_var,
            });
        }
        for b in 0..ni_m {
            let i_var = arena.insert(Term::Variable((ni_m - 1 - b) as u32));
            major_ty = arena.insert(Term::Apply {
                function: major_ty,
                argument: i_var,
            });
        }
        let mut ty = arena.insert(Term::Pi {
            domain: major_ty,
            codomain: motive_sort,
        });
        for (b, &idx_ty) in index_types_m.iter().enumerate().rev() {
            let shifted_idx = shift(arena, idx_ty, b as u32, m as u32);
            ty = arena.insert(Term::Pi {
                domain: shifted_idx,
                codomain: ty,
            });
        }
        motive_tys.push(ty);
    }

    // Build minor types `Minor_0 .. Minor_{K-1}`.
    let mut minor_tys = Vec::with_capacity(nc);
    for (c, ctor) in ctors.iter().enumerate() {
        let extra = (nm + c) as u32; // P_0..P_{M-1} + Minor_0..Minor_{c-1}
        let nf = ctor.field_types.len();
        let nr = ctor.recursive_fields.len();

        // Codomain of Minor_c: P_{target} out'_0..out'_{ni-1} (C_c p... f...)
        let motive_var = arena.insert(Term::Variable(
            (nr + nf + c + (nm - 1 - ctor.target_inductive)) as u32,
        ));
        let mut codomain = motive_var;
        for &out_idx in &ctor.output_indices {
            let shifted_p = shift(arena, out_idx, nf as u32, extra);
            let shifted_all = shift(arena, shifted_p, 0, nr as u32);
            codomain = arena.insert(Term::Apply {
                function: codomain,
                argument: shifted_all,
            });
        }
        let mut ctor_app = arena.insert(Term::Constant {
            declaration: base + (nm + c) as u32,
            levels: id_levels.clone(),
        });
        for a in 0..np {
            let p_var = arena.insert(Term::Variable(
                (nr + nf + extra as usize + (np - 1 - a)) as u32,
            ));
            ctor_app = arena.insert(Term::Apply {
                function: ctor_app,
                argument: p_var,
            });
        }
        for j in 0..nf {
            let f_var = arena.insert(Term::Variable((nr + (nf - 1 - j)) as u32));
            ctor_app = arena.insert(Term::Apply {
                function: ctor_app,
                argument: f_var,
            });
        }
        codomain = arena.insert(Term::Apply {
            function: codomain,
            argument: ctor_app,
        });

        // Wrap IH binders from innermost `nr - 1` down to `0`.
        let mut minor = codomain;
        for (q, rec) in ctor.recursive_fields.iter().enumerate().rev() {
            let j = rec.field_index;
            let r = rec.arg_binders.len();
            let after_j = ((nf - j) + q) as u32;

            // IH codomain: P_{rec_m} idx'_0..idx'_{ni-1} (f_j x_0..x_{r-1})
            let mut ih = arena.insert(Term::Variable(
                (r + q + nf + c + (nm - 1 - rec.target_inductive)) as u32,
            ));
            for &idx_arg in &rec.index_args {
                let s1 = shift(arena, idx_arg, (r + j) as u32, extra);
                let s2 = shift(arena, s1, r as u32, after_j);
                ih = arena.insert(Term::Apply {
                    function: ih,
                    argument: s2,
                });
            }
            let mut f_app = arena.insert(Term::Variable((r + q + (nf - 1 - j)) as u32));
            for m in 0..r {
                let x_var = arena.insert(Term::Variable((r - 1 - m) as u32));
                f_app = arena.insert(Term::Apply {
                    function: f_app,
                    argument: x_var,
                });
            }
            ih = arena.insert(Term::Apply {
                function: ih,
                argument: f_app,
            });
            for (m, &arg_dom) in rec.arg_binders.iter().enumerate().rev() {
                let s1 = shift(arena, arg_dom, (m + j) as u32, extra);
                let s2 = shift(arena, s1, m as u32, after_j);
                ih = arena.insert(Term::Pi {
                    domain: s2,
                    codomain: ih,
                });
            }
            minor = arena.insert(Term::Pi {
                domain: ih,
                codomain: minor,
            });
        }

        // Wrap field binders `f_{nf-1}` down to `f_0`.
        for (j, &field_ty) in ctor.field_types.iter().enumerate().rev() {
            let shifted_field = shift(arena, field_ty, j as u32, extra);
            minor = arena.insert(Term::Pi {
                domain: shifted_field,
                codomain: minor,
            });
        }
        minor_tys.push(minor);
    }

    // Build tail for `I_{target_m}`:
    // Π(i_0 : I'_0)...Π(i_{ni-1} : I'_{ni-1}). Π(major : I_{target_m} p... i...). P_{target_m} i... major
    let target_index_types = &all_index_types[target_m];
    let ni = target_index_types.len();
    let m_and_minors = (nm + nc) as u32;
    let mut result = arena.insert(Term::Variable((1 + ni + nc + (nm - 1 - target_m)) as u32));
    for b in 0..ni {
        let i_var = arena.insert(Term::Variable((1 + (ni - 1 - b)) as u32));
        result = arena.insert(Term::Apply {
            function: result,
            argument: i_var,
        });
    }
    let major_var = arena.insert(Term::Variable(0));
    result = arena.insert(Term::Apply {
        function: result,
        argument: major_var,
    });

    let mut major_domain = arena.insert(Term::Constant {
        declaration: base + target_m as u32,
        levels: id_levels,
    });
    for a in 0..np {
        let p_var = arena.insert(Term::Variable(
            (ni + m_and_minors as usize + (np - 1 - a)) as u32,
        ));
        major_domain = arena.insert(Term::Apply {
            function: major_domain,
            argument: p_var,
        });
    }
    for b in 0..ni {
        let i_var = arena.insert(Term::Variable((ni - 1 - b) as u32));
        major_domain = arena.insert(Term::Apply {
            function: major_domain,
            argument: i_var,
        });
    }
    result = arena.insert(Term::Pi {
        domain: major_domain,
        codomain: result,
    });

    for (b, &idx_ty) in target_index_types.iter().enumerate().rev() {
        let shifted_idx = shift(arena, idx_ty, b as u32, m_and_minors);
        result = arena.insert(Term::Pi {
            domain: shifted_idx,
            codomain: result,
        });
    }

    for &minor_ty in minor_tys.iter().rev() {
        result = arena.insert(Term::Pi {
            domain: minor_ty,
            codomain: result,
        });
    }
    for &motive_ty in motive_tys.iter().rev() {
        result = arena.insert(Term::Pi {
            domain: motive_ty,
            codomain: result,
        });
    }
    for &param_ty in param_types.iter().rev() {
        result = arena.insert(Term::Pi {
            domain: param_ty,
            codomain: result,
        });
    }
    result
}

/// Match a weak-head normalized major premise against a constructor of `recursor.inductive`.
pub(crate) fn match_constructor(
    arena: &TermArena,
    signature: &Signature,
    recursor: &RecursorInfo,
    major_whnf: TermHandle,
) -> Option<(usize, Vec<TermHandle>)> {
    let (head, args) = unwind_app(arena, major_whnf);
    let Term::Constant { declaration, .. } = arena.get(head) else {
        return None;
    };
    let cinfo = signature.get_constructor(declaration)?;
    if cinfo.inductive != recursor.inductive || args.len() != cinfo.num_params + cinfo.num_fields {
        return None;
    }
    Some((cinfo.constructor_index, args[cinfo.num_params..].to_vec()))
}

fn instantiate_under(
    arena: &mut TermArena,
    term: TermHandle,
    depth: usize,
    arguments: &[TermHandle],
) -> TermHandle {
    if arguments.is_empty() {
        return term;
    }
    if depth == 0 {
        return instantiate(arena, term, arguments);
    }
    let dummy = arena.insert(Term::Two);
    let mut wrapped = term;
    for _ in 0..depth {
        wrapped = arena.insert(Term::Lambda {
            domain: dummy,
            body: wrapped,
        });
    }
    let mut result = instantiate(arena, wrapped, arguments);
    for _ in 0..depth {
        let Term::Lambda { body, .. } = arena.get(result) else {
            unreachable!("instantiate preserves outer lambda binders");
        };
        result = body;
    }
    result
}

/// Perform 1-step iota-reduction for
/// `I_m.rec[levels] params... motives... minors... indices... (C_k params... fields...)`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn reduce_recursor(
    arena: &mut TermArena,
    _recursor_decl: u32,
    rec_levels: &[Level],
    recursor: &RecursorInfo,
    rec_prefix: Option<TermHandle>,
    rec_args: &[TermHandle],
    ctor_index: usize,
    ctor_fields: &[TermHandle],
) -> Result<TermHandle, CoreError> {
    let np = recursor.num_params;
    let nm = recursor.num_mutual;
    let nc = recursor.num_minors;
    let prefix_len = np + nm + nc;
    let params = &rec_args[..np];
    let minors = &rec_args[np + nm..prefix_len];
    let inductive_levels = &rec_levels[..recursor.inductive_level_arity as usize];
    let rule = &recursor.rules[ctor_index];

    let mut result = minors[ctor_index];
    for &field in ctor_fields {
        result = arena.insert(Term::Apply {
            function: result,
            argument: field,
        });
    }

    let mut target_prefixes: Vec<Option<TermHandle>> = vec![None; nm];
    target_prefixes[(recursor.inductive - recursor.block_base) as usize] = rec_prefix;

    for rec_field in &rule.recursive_fields {
        let j = rec_field.field_index;
        let r = rec_field.arg_binders.len();
        let field_val = ctor_fields[j];
        let target_m = rec_field.target_inductive;
        let base_prefix = match target_prefixes[target_m] {
            Some(prefix) => prefix,
            None => {
                let target_rec_decl = recursor.block_base + (nm + nc + target_m) as u32;
                let mut prefix = arena.insert(Term::Constant {
                    declaration: target_rec_decl,
                    levels: rec_levels.to_vec(),
                });
                for &arg in &rec_args[..prefix_len] {
                    prefix = arena.insert(Term::Apply {
                        function: prefix,
                        argument: arg,
                    });
                }
                target_prefixes[target_m] = Some(prefix);
                prefix
            }
        };

        let mut subst_args = Vec::with_capacity(np + j);
        subst_args.extend_from_slice(params);
        subst_args.extend_from_slice(&ctor_fields[..j]);

        let mut ih = shift(arena, base_prefix, 0, r as u32);
        for &idx_template in &rec_field.index_args {
            let leveled =
                instantiate_levels(arena, idx_template, inductive_levels).map_err(|index| {
                    CoreError::UnboundLevelParameter {
                        index,
                        arity: recursor.inductive_level_arity,
                    }
                })?;
            let inst_idx = instantiate_under(arena, leveled, r, &subst_args);
            ih = arena.insert(Term::Apply {
                function: ih,
                argument: inst_idx,
            });
        }
        let mut child = shift(arena, field_val, 0, r as u32);
        for m in 0..r {
            let x_var = arena.insert(Term::Variable((r - 1 - m) as u32));
            child = arena.insert(Term::Apply {
                function: child,
                argument: x_var,
            });
        }
        ih = arena.insert(Term::Apply {
            function: ih,
            argument: child,
        });

        for (m, &arg_dom) in rec_field.arg_binders.iter().enumerate().rev() {
            let leveled =
                instantiate_levels(arena, arg_dom, inductive_levels).map_err(|index| {
                    CoreError::UnboundLevelParameter {
                        index,
                        arity: recursor.inductive_level_arity,
                    }
                })?;
            let dom = instantiate_under(arena, leveled, m, &subst_args);
            ih = arena.insert(Term::Lambda {
                domain: dom,
                body: ih,
            });
        }

        result = arena.insert(Term::Apply {
            function: result,
            argument: ih,
        });
    }

    Ok(result)
}

/// Build the `2 + constructor_tys.len()` signature declarations `[I, C_0, ..., C_{k-1}, I.rec]`
/// for a single inductive family placed at `base`.
pub fn declare_inductive_family(
    arena: &mut TermArena,
    base: u32,
    level_arity: u32,
    num_params: u32,
    inductive_ty: TermHandle,
    constructor_tys: &[TermHandle],
) -> Vec<Declaration> {
    let ctors: Vec<(usize, TermHandle)> = constructor_tys.iter().map(|&ty| (0, ty)).collect();
    declare_mutual_inductive_family(
        arena,
        base,
        level_arity,
        num_params,
        &[inductive_ty],
        &ctors,
    )
}

/// Build the `2*M + K` signature declarations
/// `[I_0, ..., I_{M-1}, C_0, ..., C_{K-1}, I_0.rec, ..., I_{M-1}.rec]`
/// for a mutually recursive inductive block placed at `base`.
/// Each entry in `constructors` is `(target_mutual_index, constructor_type)`.
/// `check_signature` still validates everything.
pub fn declare_mutual_inductive_family(
    arena: &mut TermArena,
    base: u32,
    level_arity: u32,
    num_params: u32,
    inductive_tys: &[TermHandle],
    constructors: &[(usize, TermHandle)],
) -> Vec<Declaration> {
    let nm = inductive_tys.len();
    let nc = constructors.len();
    let np = num_params as usize;

    let mut param_types = Vec::new();
    let mut all_index_types = Vec::with_capacity(nm);
    let mut split = np;
    for (m, &ind_ty) in inductive_tys.iter().enumerate() {
        let mut binders = Vec::new();
        let mut cur = ind_ty;
        while let Term::Pi { domain, codomain } = arena.get(cur) {
            binders.push(domain);
            cur = codomain;
        }
        let s = np.min(binders.len());
        let index_types = binders.split_off(s);
        if m == 0 {
            split = s;
            param_types = binders;
        }
        all_index_types.push(index_types);
    }

    let mut parsed_ctors = Vec::with_capacity(nc);
    for &(target_inductive, ctor_ty) in constructors {
        let mut cur = ctor_ty;
        for _ in 0..split {
            if let Term::Pi { codomain, .. } = arena.get(cur) {
                cur = codomain;
            }
        }
        let mut field_types = Vec::new();
        let mut recursive_fields = Vec::new();
        while let Term::Pi { domain, codomain } = arena.get(cur) {
            let field_index = field_types.len();
            if contains_any_inductive(arena, domain, base, nm as u32) {
                let mut arg_binders = Vec::new();
                let mut fcur = domain;
                while let Term::Pi {
                    domain: ad,
                    codomain: ac,
                } = arena.get(fcur)
                {
                    arg_binders.push(ad);
                    fcur = ac;
                }
                let (head, args) = unwind_app(arena, fcur);
                let rec_m = match arena.get(head) {
                    Term::Constant { declaration, .. }
                        if declaration >= base && declaration < base + nm as u32 =>
                    {
                        (declaration - base) as usize
                    }
                    _ => 0,
                };
                let index_args = if args.len() >= split {
                    args[split..].to_vec()
                } else {
                    Vec::new()
                };
                recursive_fields.push(RecursiveFieldSpec {
                    field_index,
                    target_inductive: rec_m,
                    arg_binders,
                    index_args,
                });
            }
            field_types.push(domain);
            cur = codomain;
        }
        let (_, args) = unwind_app(arena, cur);
        let output_indices = if args.len() >= split {
            args[split..].to_vec()
        } else {
            Vec::new()
        };
        parsed_ctors.push(ParsedConstructor {
            target_inductive,
            field_types,
            recursive_fields,
            output_indices,
        });
    }

    let mut declarations = Vec::with_capacity(2 * nm + nc);
    for (m, &ind_ty) in inductive_tys.iter().enumerate() {
        declarations.push(Declaration::mutual_inductive(
            level_arity,
            nm as u32,
            m as u32,
            num_params,
            nc as u32,
            ind_ty,
        ));
    }
    for (c, &(target_m, ctor_ty)) in constructors.iter().enumerate() {
        declarations.push(Declaration::constructor(
            level_arity,
            base + target_m as u32,
            c as u32,
            ctor_ty,
        ));
    }
    for m in 0..nm {
        let rec_ty = build_recursor_type(
            arena,
            base,
            m,
            level_arity,
            &param_types,
            &all_index_types,
            &parsed_ctors,
        );
        declarations.push(Declaration::recursor(
            level_arity + 1,
            base + m as u32,
            rec_ty,
        ));
    }
    declarations
}
