//! Capture-avoiding de Bruijn operations. Both functions return the original
//! handle when no node changes, so unchanged subterms keep their storage.

use super::term::{Level, Sort, Term, TermArena, TermHandle, instantiate_level};

/// Increase every free variable at or above `cutoff` by `amount`. Returns
/// `term` unchanged when `amount` is zero or no variable qualifies.
pub fn shift(arena: &mut TermArena, term: TermHandle, cutoff: u32, amount: u32) -> TermHandle {
    if amount == 0 {
        return term;
    }
    match arena.get(term) {
        Term::Variable(index) => {
            if index >= cutoff {
                arena.insert(Term::Variable(
                    index.checked_add(amount).expect("variable shift overflow"),
                ))
            } else {
                term
            }
        }
        Term::Pi { domain, codomain } => {
            let shifted_domain = shift(arena, domain, cutoff, amount);
            let shifted_codomain = shift(arena, codomain, cutoff + 1, amount);
            if shifted_domain == domain && shifted_codomain == codomain {
                return term;
            }
            arena.insert(Term::Pi {
                domain: shifted_domain,
                codomain: shifted_codomain,
            })
        }
        Term::Lambda { domain, body } => {
            let shifted_domain = shift(arena, domain, cutoff, amount);
            let shifted_body = shift(arena, body, cutoff + 1, amount);
            if shifted_domain == domain && shifted_body == body {
                return term;
            }
            arena.insert(Term::Lambda {
                domain: shifted_domain,
                body: shifted_body,
            })
        }
        Term::Apply { function, argument } => {
            let shifted_function = shift(arena, function, cutoff, amount);
            let shifted_argument = shift(arena, argument, cutoff, amount);
            if shifted_function == function && shifted_argument == argument {
                return term;
            }
            arena.insert(Term::Apply {
                function: shifted_function,
                argument: shifted_argument,
            })
        }
        Term::Sigma { domain, codomain } => {
            let shifted_domain = shift(arena, domain, cutoff, amount);
            let shifted_codomain = shift(arena, codomain, cutoff + 1, amount);
            if shifted_domain == domain && shifted_codomain == codomain {
                return term;
            }
            arena.insert(Term::Sigma {
                domain: shifted_domain,
                codomain: shifted_codomain,
            })
        }
        Term::Pair { first, second } => {
            let shifted_first = shift(arena, first, cutoff, amount);
            let shifted_second = shift(arena, second, cutoff, amount);
            if shifted_first == first && shifted_second == second {
                return term;
            }
            arena.insert(Term::Pair {
                first: shifted_first,
                second: shifted_second,
            })
        }
        Term::Fst { pair } => {
            let shifted_pair = shift(arena, pair, cutoff, amount);
            if shifted_pair == pair {
                return term;
            }
            arena.insert(Term::Fst { pair: shifted_pair })
        }
        Term::Snd { pair } => {
            let shifted_pair = shift(arena, pair, cutoff, amount);
            if shifted_pair == pair {
                return term;
            }
            arena.insert(Term::Snd { pair: shifted_pair })
        }
        Term::CaseTwo {
            motive,
            zero_branch,
            one_branch,
            scrutinee,
        } => {
            let shifted_motive = shift(arena, motive, cutoff, amount);
            let shifted_zero_branch = shift(arena, zero_branch, cutoff, amount);
            let shifted_one_branch = shift(arena, one_branch, cutoff, amount);
            let shifted_scrutinee = shift(arena, scrutinee, cutoff, amount);
            if shifted_motive == motive
                && shifted_zero_branch == zero_branch
                && shifted_one_branch == one_branch
                && shifted_scrutinee == scrutinee
            {
                return term;
            }
            arena.insert(Term::CaseTwo {
                motive: shifted_motive,
                zero_branch: shifted_zero_branch,
                one_branch: shifted_one_branch,
                scrutinee: shifted_scrutinee,
            })
        }
        Term::Id { ty, left, right } => {
            let shifted_ty = shift(arena, ty, cutoff, amount);
            let shifted_left = shift(arena, left, cutoff, amount);
            let shifted_right = shift(arena, right, cutoff, amount);
            if shifted_ty == ty && shifted_left == left && shifted_right == right {
                return term;
            }
            arena.insert(Term::Id {
                ty: shifted_ty,
                left: shifted_left,
                right: shifted_right,
            })
        }
        Term::Refl { ty, value } => {
            let shifted_ty = shift(arena, ty, cutoff, amount);
            let shifted_value = shift(arena, value, cutoff, amount);
            if shifted_ty == ty && shifted_value == value {
                return term;
            }
            arena.insert(Term::Refl {
                ty: shifted_ty,
                value: shifted_value,
            })
        }
        Term::IdElim {
            motive,
            base,
            endpoint,
            proof,
        } => {
            let shifted_motive = shift(arena, motive, cutoff, amount);
            let shifted_base = shift(arena, base, cutoff, amount);
            let shifted_endpoint = shift(arena, endpoint, cutoff, amount);
            let shifted_proof = shift(arena, proof, cutoff, amount);
            if shifted_motive == motive
                && shifted_base == base
                && shifted_endpoint == endpoint
                && shifted_proof == proof
            {
                return term;
            }
            arena.insert(Term::IdElim {
                motive: shifted_motive,
                base: shifted_base,
                endpoint: shifted_endpoint,
                proof: shifted_proof,
            })
        }
        Term::W { carrier, children } => {
            let shifted_carrier = shift(arena, carrier, cutoff, amount);
            let shifted_children = shift(arena, children, cutoff, amount);
            if shifted_carrier == carrier && shifted_children == children {
                return term;
            }
            arena.insert(Term::W {
                carrier: shifted_carrier,
                children: shifted_children,
            })
        }
        Term::Sup {
            carrier,
            children,
            label,
            function,
        } => {
            let shifted_carrier = shift(arena, carrier, cutoff, amount);
            let shifted_children = shift(arena, children, cutoff, amount);
            let shifted_label = shift(arena, label, cutoff, amount);
            let shifted_function = shift(arena, function, cutoff, amount);
            if shifted_carrier == carrier
                && shifted_children == children
                && shifted_label == label
                && shifted_function == function
            {
                return term;
            }
            arena.insert(Term::Sup {
                carrier: shifted_carrier,
                children: shifted_children,
                label: shifted_label,
                function: shifted_function,
            })
        }
        Term::IndW { motive, step, tree } => {
            let shifted_motive = shift(arena, motive, cutoff, amount);
            let shifted_step = shift(arena, step, cutoff, amount);
            let shifted_tree = shift(arena, tree, cutoff, amount);
            if shifted_motive == motive && shifted_step == step && shifted_tree == tree {
                return term;
            }
            arena.insert(Term::IndW {
                motive: shifted_motive,
                step: shifted_step,
                tree: shifted_tree,
            })
        }
        Term::EmptyElim { ty, scrutinee } => {
            let shifted_ty = shift(arena, ty, cutoff, amount);
            let shifted_scrutinee = shift(arena, scrutinee, cutoff, amount);
            if shifted_ty == ty && shifted_scrutinee == scrutinee {
                return term;
            }
            arena.insert(Term::EmptyElim {
                ty: shifted_ty,
                scrutinee: shifted_scrutinee,
            })
        }
        Term::Squash { ty } => {
            let shifted_ty = shift(arena, ty, cutoff, amount);
            if shifted_ty == ty {
                return term;
            }
            arena.insert(Term::Squash { ty: shifted_ty })
        }
        Term::SquashIntro { ty, value } => {
            let shifted_ty = shift(arena, ty, cutoff, amount);
            let shifted_value = shift(arena, value, cutoff, amount);
            if shifted_ty == ty && shifted_value == value {
                return term;
            }
            arena.insert(Term::SquashIntro {
                ty: shifted_ty,
                value: shifted_value,
            })
        }
        Term::SquashElim {
            proposition,
            function,
            scrutinee,
        } => {
            let shifted_proposition = shift(arena, proposition, cutoff, amount);
            let shifted_function = shift(arena, function, cutoff, amount);
            let shifted_scrutinee = shift(arena, scrutinee, cutoff, amount);
            if shifted_proposition == proposition
                && shifted_function == function
                && shifted_scrutinee == scrutinee
            {
                return term;
            }
            arena.insert(Term::SquashElim {
                proposition: shifted_proposition,
                function: shifted_function,
                scrutinee: shifted_scrutinee,
            })
        }
        Term::Box { ty } => {
            let shifted_ty = shift(arena, ty, cutoff, amount);
            if shifted_ty == ty {
                return term;
            }
            arena.insert(Term::Box { ty: shifted_ty })
        }
        Term::BoxIntro { ty, value } => {
            let shifted_ty = shift(arena, ty, cutoff, amount);
            let shifted_value = shift(arena, value, cutoff, amount);
            if shifted_ty == ty && shifted_value == value {
                return term;
            }
            arena.insert(Term::BoxIntro {
                ty: shifted_ty,
                value: shifted_value,
            })
        }
        Term::BoxElim {
            motive,
            body,
            scrutinee,
        } => {
            let shifted_motive = shift(arena, motive, cutoff, amount);
            let shifted_body = shift(arena, body, cutoff, amount);
            let shifted_scrutinee = shift(arena, scrutinee, cutoff, amount);
            if shifted_motive == motive && shifted_body == body && shifted_scrutinee == scrutinee {
                return term;
            }
            arena.insert(Term::BoxElim {
                motive: shifted_motive,
                body: shifted_body,
                scrutinee: shifted_scrutinee,
            })
        }
        // A constant's level arguments are judgment-scope level
        // expressions, not terms: binders never bind them, so a term
        // shift leaves a constant untouched.
        Term::Sort(_)
        | Term::Two
        | Term::TwoZero
        | Term::TwoOne
        | Term::Empty
        | Term::Dummy
        | Term::Constant { .. } => term,
    }
}

/// Replace de Bruijn index 0 in `body` with `argument`, shifting `argument`
/// under each binder it passes and decrementing the remaining free variables
/// of `body`. Returns `body` unchanged when index 0 does not occur.
pub fn substitute(arena: &mut TermArena, body: TermHandle, argument: TermHandle) -> TermHandle {
    substitute_at(arena, body, argument, 0)
}

fn substitute_at(
    arena: &mut TermArena,
    term: TermHandle,
    argument: TermHandle,
    depth: u32,
) -> TermHandle {
    match arena.get(term) {
        Term::Variable(index) => match index.cmp(&depth) {
            std::cmp::Ordering::Equal => shift(arena, argument, 0, depth),
            std::cmp::Ordering::Greater => arena.insert(Term::Variable(index - 1)),
            std::cmp::Ordering::Less => term,
        },
        Term::Pi { domain, codomain } => {
            let new_domain = substitute_at(arena, domain, argument, depth);
            let new_codomain = substitute_at(arena, codomain, argument, depth + 1);
            if new_domain == domain && new_codomain == codomain {
                return term;
            }
            arena.insert(Term::Pi {
                domain: new_domain,
                codomain: new_codomain,
            })
        }
        Term::Lambda { domain, body } => {
            let new_domain = substitute_at(arena, domain, argument, depth);
            let new_body = substitute_at(arena, body, argument, depth + 1);
            if new_domain == domain && new_body == body {
                return term;
            }
            arena.insert(Term::Lambda {
                domain: new_domain,
                body: new_body,
            })
        }
        Term::Apply {
            function,
            argument: operand,
        } => {
            let new_function = substitute_at(arena, function, argument, depth);
            let new_operand = substitute_at(arena, operand, argument, depth);
            if new_function == function && new_operand == operand {
                return term;
            }
            arena.insert(Term::Apply {
                function: new_function,
                argument: new_operand,
            })
        }
        Term::Sigma { domain, codomain } => {
            let new_domain = substitute_at(arena, domain, argument, depth);
            let new_codomain = substitute_at(arena, codomain, argument, depth + 1);
            if new_domain == domain && new_codomain == codomain {
                return term;
            }
            arena.insert(Term::Sigma {
                domain: new_domain,
                codomain: new_codomain,
            })
        }
        Term::Pair { first, second } => {
            let new_first = substitute_at(arena, first, argument, depth);
            let new_second = substitute_at(arena, second, argument, depth);
            if new_first == first && new_second == second {
                return term;
            }
            arena.insert(Term::Pair {
                first: new_first,
                second: new_second,
            })
        }
        Term::Fst { pair } => {
            let new_pair = substitute_at(arena, pair, argument, depth);
            if new_pair == pair {
                return term;
            }
            arena.insert(Term::Fst { pair: new_pair })
        }
        Term::Snd { pair } => {
            let new_pair = substitute_at(arena, pair, argument, depth);
            if new_pair == pair {
                return term;
            }
            arena.insert(Term::Snd { pair: new_pair })
        }
        Term::CaseTwo {
            motive,
            zero_branch,
            one_branch,
            scrutinee,
        } => {
            let new_motive = substitute_at(arena, motive, argument, depth);
            let new_zero_branch = substitute_at(arena, zero_branch, argument, depth);
            let new_one_branch = substitute_at(arena, one_branch, argument, depth);
            let new_scrutinee = substitute_at(arena, scrutinee, argument, depth);
            if new_motive == motive
                && new_zero_branch == zero_branch
                && new_one_branch == one_branch
                && new_scrutinee == scrutinee
            {
                return term;
            }
            arena.insert(Term::CaseTwo {
                motive: new_motive,
                zero_branch: new_zero_branch,
                one_branch: new_one_branch,
                scrutinee: new_scrutinee,
            })
        }
        Term::Id { ty, left, right } => {
            let new_ty = substitute_at(arena, ty, argument, depth);
            let new_left = substitute_at(arena, left, argument, depth);
            let new_right = substitute_at(arena, right, argument, depth);
            if new_ty == ty && new_left == left && new_right == right {
                return term;
            }
            arena.insert(Term::Id {
                ty: new_ty,
                left: new_left,
                right: new_right,
            })
        }
        Term::Refl { ty, value } => {
            let new_ty = substitute_at(arena, ty, argument, depth);
            let new_value = substitute_at(arena, value, argument, depth);
            if new_ty == ty && new_value == value {
                return term;
            }
            arena.insert(Term::Refl {
                ty: new_ty,
                value: new_value,
            })
        }
        Term::IdElim {
            motive,
            base,
            endpoint,
            proof,
        } => {
            let new_motive = substitute_at(arena, motive, argument, depth);
            let new_base = substitute_at(arena, base, argument, depth);
            let new_endpoint = substitute_at(arena, endpoint, argument, depth);
            let new_proof = substitute_at(arena, proof, argument, depth);
            if new_motive == motive
                && new_base == base
                && new_endpoint == endpoint
                && new_proof == proof
            {
                return term;
            }
            arena.insert(Term::IdElim {
                motive: new_motive,
                base: new_base,
                endpoint: new_endpoint,
                proof: new_proof,
            })
        }
        Term::W { carrier, children } => {
            let new_carrier = substitute_at(arena, carrier, argument, depth);
            let new_children = substitute_at(arena, children, argument, depth);
            if new_carrier == carrier && new_children == children {
                return term;
            }
            arena.insert(Term::W {
                carrier: new_carrier,
                children: new_children,
            })
        }
        Term::Sup {
            carrier,
            children,
            label,
            function,
        } => {
            let new_carrier = substitute_at(arena, carrier, argument, depth);
            let new_children = substitute_at(arena, children, argument, depth);
            let new_label = substitute_at(arena, label, argument, depth);
            let new_function = substitute_at(arena, function, argument, depth);
            if new_carrier == carrier
                && new_children == children
                && new_label == label
                && new_function == function
            {
                return term;
            }
            arena.insert(Term::Sup {
                carrier: new_carrier,
                children: new_children,
                label: new_label,
                function: new_function,
            })
        }
        Term::IndW { motive, step, tree } => {
            let new_motive = substitute_at(arena, motive, argument, depth);
            let new_step = substitute_at(arena, step, argument, depth);
            let new_tree = substitute_at(arena, tree, argument, depth);
            if new_motive == motive && new_step == step && new_tree == tree {
                return term;
            }
            arena.insert(Term::IndW {
                motive: new_motive,
                step: new_step,
                tree: new_tree,
            })
        }
        Term::EmptyElim { ty, scrutinee } => {
            let new_ty = substitute_at(arena, ty, argument, depth);
            let new_scrutinee = substitute_at(arena, scrutinee, argument, depth);
            if new_ty == ty && new_scrutinee == scrutinee {
                return term;
            }
            arena.insert(Term::EmptyElim {
                ty: new_ty,
                scrutinee: new_scrutinee,
            })
        }
        Term::Squash { ty } => {
            let new_ty = substitute_at(arena, ty, argument, depth);
            if new_ty == ty {
                return term;
            }
            arena.insert(Term::Squash { ty: new_ty })
        }
        Term::SquashIntro { ty, value } => {
            let new_ty = substitute_at(arena, ty, argument, depth);
            let new_value = substitute_at(arena, value, argument, depth);
            if new_ty == ty && new_value == value {
                return term;
            }
            arena.insert(Term::SquashIntro {
                ty: new_ty,
                value: new_value,
            })
        }
        Term::SquashElim {
            proposition,
            function,
            scrutinee,
        } => {
            let new_proposition = substitute_at(arena, proposition, argument, depth);
            let new_function = substitute_at(arena, function, argument, depth);
            let new_scrutinee = substitute_at(arena, scrutinee, argument, depth);
            if new_proposition == proposition
                && new_function == function
                && new_scrutinee == scrutinee
            {
                return term;
            }
            arena.insert(Term::SquashElim {
                proposition: new_proposition,
                function: new_function,
                scrutinee: new_scrutinee,
            })
        }
        Term::Box { ty } => {
            let new_ty = substitute_at(arena, ty, argument, depth);
            if new_ty == ty {
                return term;
            }
            arena.insert(Term::Box { ty: new_ty })
        }
        Term::BoxIntro { ty, value } => {
            let new_ty = substitute_at(arena, ty, argument, depth);
            let new_value = substitute_at(arena, value, argument, depth);
            if new_ty == ty && new_value == value {
                return term;
            }
            arena.insert(Term::BoxIntro {
                ty: new_ty,
                value: new_value,
            })
        }
        Term::BoxElim {
            motive,
            body,
            scrutinee,
        } => {
            let new_motive = substitute_at(arena, motive, argument, depth);
            let new_body = substitute_at(arena, body, argument, depth);
            let new_scrutinee = substitute_at(arena, scrutinee, argument, depth);
            if new_motive == motive && new_body == body && new_scrutinee == scrutinee {
                return term;
            }
            arena.insert(Term::BoxElim {
                motive: new_motive,
                body: new_body,
                scrutinee: new_scrutinee,
            })
        }
        // Level arguments are not de Bruijn terms — term substitution
        // never reaches them.
        Term::Sort(_)
        | Term::Two
        | Term::TwoZero
        | Term::TwoOne
        | Term::Empty
        | Term::Dummy
        | Term::Constant { .. } => term,
    }
}

/// Instantiate the universe parameters of `term` at `arguments`: every
/// `Parameter(i)` inside a `Sort` payload or a nested `Constant`'s level
/// arguments becomes `arguments[i]`. This is the substitution half of the
/// declaration rule — a declaration checked at arity `n` is sound at
/// `arguments` of length `n` because the checking judgment was
/// parametric. Returns the same handle where nothing changed, so
/// instantiated subterms keep their storage.
///
/// `Err(index)` reports a parameter out of range of `arguments`: typing
/// scope-checks the declaration's own arity and every instantiation's
/// argument list before this runs, so reaching it means the signature or
/// term was malformed input rather than a checked judgment.
pub fn instantiate_levels(
    arena: &mut TermArena,
    term: TermHandle,
    arguments: &[Level],
) -> Result<TermHandle, u32> {
    match arena.get(term) {
        Term::Sort(sort) => {
            let level = instantiate_level(&sort.level(), arguments)?;
            let sort = match sort {
                Sort::Type(_) => Sort::Type(level),
                Sort::Strict(_) => Sort::Strict(level),
            };
            Ok(arena.insert(Term::Sort(sort)))
        }
        Term::Constant {
            declaration,
            levels,
        } => {
            let mut instantiated = Vec::with_capacity(levels.len());
            for level in &levels {
                instantiated.push(instantiate_level(level, arguments)?);
            }
            Ok(arena.insert(Term::Constant {
                declaration,
                levels: instantiated,
            }))
        }
        Term::Pi { domain, codomain } => {
            let new_domain = instantiate_levels(arena, domain, arguments)?;
            let new_codomain = instantiate_levels(arena, codomain, arguments)?;
            if new_domain == domain && new_codomain == codomain {
                return Ok(term);
            }
            Ok(arena.insert(Term::Pi {
                domain: new_domain,
                codomain: new_codomain,
            }))
        }
        Term::Lambda { domain, body } => {
            let new_domain = instantiate_levels(arena, domain, arguments)?;
            let new_body = instantiate_levels(arena, body, arguments)?;
            if new_domain == domain && new_body == body {
                return Ok(term);
            }
            Ok(arena.insert(Term::Lambda {
                domain: new_domain,
                body: new_body,
            }))
        }
        Term::Apply { function, argument } => {
            let new_function = instantiate_levels(arena, function, arguments)?;
            let new_argument = instantiate_levels(arena, argument, arguments)?;
            if new_function == function && new_argument == argument {
                return Ok(term);
            }
            Ok(arena.insert(Term::Apply {
                function: new_function,
                argument: new_argument,
            }))
        }
        Term::Sigma { domain, codomain } => {
            let new_domain = instantiate_levels(arena, domain, arguments)?;
            let new_codomain = instantiate_levels(arena, codomain, arguments)?;
            if new_domain == domain && new_codomain == codomain {
                return Ok(term);
            }
            Ok(arena.insert(Term::Sigma {
                domain: new_domain,
                codomain: new_codomain,
            }))
        }
        Term::Pair { first, second } => {
            let new_first = instantiate_levels(arena, first, arguments)?;
            let new_second = instantiate_levels(arena, second, arguments)?;
            if new_first == first && new_second == second {
                return Ok(term);
            }
            Ok(arena.insert(Term::Pair {
                first: new_first,
                second: new_second,
            }))
        }
        Term::Fst { pair } => {
            let new_pair = instantiate_levels(arena, pair, arguments)?;
            if new_pair == pair {
                return Ok(term);
            }
            Ok(arena.insert(Term::Fst { pair: new_pair }))
        }
        Term::Snd { pair } => {
            let new_pair = instantiate_levels(arena, pair, arguments)?;
            if new_pair == pair {
                return Ok(term);
            }
            Ok(arena.insert(Term::Snd { pair: new_pair }))
        }
        Term::CaseTwo {
            motive,
            zero_branch,
            one_branch,
            scrutinee,
        } => {
            let new_motive = instantiate_levels(arena, motive, arguments)?;
            let new_zero_branch = instantiate_levels(arena, zero_branch, arguments)?;
            let new_one_branch = instantiate_levels(arena, one_branch, arguments)?;
            let new_scrutinee = instantiate_levels(arena, scrutinee, arguments)?;
            if new_motive == motive
                && new_zero_branch == zero_branch
                && new_one_branch == one_branch
                && new_scrutinee == scrutinee
            {
                return Ok(term);
            }
            Ok(arena.insert(Term::CaseTwo {
                motive: new_motive,
                zero_branch: new_zero_branch,
                one_branch: new_one_branch,
                scrutinee: new_scrutinee,
            }))
        }
        Term::Id { ty, left, right } => {
            let new_ty = instantiate_levels(arena, ty, arguments)?;
            let new_left = instantiate_levels(arena, left, arguments)?;
            let new_right = instantiate_levels(arena, right, arguments)?;
            if new_ty == ty && new_left == left && new_right == right {
                return Ok(term);
            }
            Ok(arena.insert(Term::Id {
                ty: new_ty,
                left: new_left,
                right: new_right,
            }))
        }
        Term::Refl { ty, value } => {
            let new_ty = instantiate_levels(arena, ty, arguments)?;
            let new_value = instantiate_levels(arena, value, arguments)?;
            if new_ty == ty && new_value == value {
                return Ok(term);
            }
            Ok(arena.insert(Term::Refl {
                ty: new_ty,
                value: new_value,
            }))
        }
        Term::IdElim {
            motive,
            base,
            endpoint,
            proof,
        } => {
            let new_motive = instantiate_levels(arena, motive, arguments)?;
            let new_base = instantiate_levels(arena, base, arguments)?;
            let new_endpoint = instantiate_levels(arena, endpoint, arguments)?;
            let new_proof = instantiate_levels(arena, proof, arguments)?;
            if new_motive == motive
                && new_base == base
                && new_endpoint == endpoint
                && new_proof == proof
            {
                return Ok(term);
            }
            Ok(arena.insert(Term::IdElim {
                motive: new_motive,
                base: new_base,
                endpoint: new_endpoint,
                proof: new_proof,
            }))
        }
        Term::W { carrier, children } => {
            let new_carrier = instantiate_levels(arena, carrier, arguments)?;
            let new_children = instantiate_levels(arena, children, arguments)?;
            if new_carrier == carrier && new_children == children {
                return Ok(term);
            }
            Ok(arena.insert(Term::W {
                carrier: new_carrier,
                children: new_children,
            }))
        }
        Term::Sup {
            carrier,
            children,
            label,
            function,
        } => {
            let new_carrier = instantiate_levels(arena, carrier, arguments)?;
            let new_children = instantiate_levels(arena, children, arguments)?;
            let new_label = instantiate_levels(arena, label, arguments)?;
            let new_function = instantiate_levels(arena, function, arguments)?;
            if new_carrier == carrier
                && new_children == children
                && new_label == label
                && new_function == function
            {
                return Ok(term);
            }
            Ok(arena.insert(Term::Sup {
                carrier: new_carrier,
                children: new_children,
                label: new_label,
                function: new_function,
            }))
        }
        Term::IndW { motive, step, tree } => {
            let new_motive = instantiate_levels(arena, motive, arguments)?;
            let new_step = instantiate_levels(arena, step, arguments)?;
            let new_tree = instantiate_levels(arena, tree, arguments)?;
            if new_motive == motive && new_step == step && new_tree == tree {
                return Ok(term);
            }
            Ok(arena.insert(Term::IndW {
                motive: new_motive,
                step: new_step,
                tree: new_tree,
            }))
        }
        Term::EmptyElim { ty, scrutinee } => {
            let new_ty = instantiate_levels(arena, ty, arguments)?;
            let new_scrutinee = instantiate_levels(arena, scrutinee, arguments)?;
            if new_ty == ty && new_scrutinee == scrutinee {
                return Ok(term);
            }
            Ok(arena.insert(Term::EmptyElim {
                ty: new_ty,
                scrutinee: new_scrutinee,
            }))
        }
        Term::Squash { ty } => {
            let new_ty = instantiate_levels(arena, ty, arguments)?;
            if new_ty == ty {
                return Ok(term);
            }
            Ok(arena.insert(Term::Squash { ty: new_ty }))
        }
        Term::SquashIntro { ty, value } => {
            let new_ty = instantiate_levels(arena, ty, arguments)?;
            let new_value = instantiate_levels(arena, value, arguments)?;
            if new_ty == ty && new_value == value {
                return Ok(term);
            }
            Ok(arena.insert(Term::SquashIntro {
                ty: new_ty,
                value: new_value,
            }))
        }
        Term::SquashElim {
            proposition,
            function,
            scrutinee,
        } => {
            let new_proposition = instantiate_levels(arena, proposition, arguments)?;
            let new_function = instantiate_levels(arena, function, arguments)?;
            let new_scrutinee = instantiate_levels(arena, scrutinee, arguments)?;
            if new_proposition == proposition
                && new_function == function
                && new_scrutinee == scrutinee
            {
                return Ok(term);
            }
            Ok(arena.insert(Term::SquashElim {
                proposition: new_proposition,
                function: new_function,
                scrutinee: new_scrutinee,
            }))
        }
        Term::Box { ty } => {
            let new_ty = instantiate_levels(arena, ty, arguments)?;
            if new_ty == ty {
                return Ok(term);
            }
            Ok(arena.insert(Term::Box { ty: new_ty }))
        }
        Term::BoxIntro { ty, value } => {
            let new_ty = instantiate_levels(arena, ty, arguments)?;
            let new_value = instantiate_levels(arena, value, arguments)?;
            if new_ty == ty && new_value == value {
                return Ok(term);
            }
            Ok(arena.insert(Term::BoxIntro {
                ty: new_ty,
                value: new_value,
            }))
        }
        Term::BoxElim {
            motive,
            body,
            scrutinee,
        } => {
            let new_motive = instantiate_levels(arena, motive, arguments)?;
            let new_body = instantiate_levels(arena, body, arguments)?;
            let new_scrutinee = instantiate_levels(arena, scrutinee, arguments)?;
            if new_motive == motive && new_body == body && new_scrutinee == scrutinee {
                return Ok(term);
            }
            Ok(arena.insert(Term::BoxElim {
                motive: new_motive,
                body: new_body,
                scrutinee: new_scrutinee,
            }))
        }
        // No remaining node can mention a level parameter.
        Term::Dummy
        | Term::Variable(_)
        | Term::Two
        | Term::TwoZero
        | Term::TwoOne
        | Term::Empty => Ok(term),
    }
}
