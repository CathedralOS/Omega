"""Scratch validator: replay the derivation checker's work counter host-side.

Mirrors bootstrap/proofs/checker/implementation charging exactly:

- derivation_provision reserves count+1.
- each derivation_row reserves 1, then rule work:
    reflexivity (rule 1):  compare(l, r)
    symmetry    (rule 2):  compare(l, p.r) + compare(r, p.l)
    transitivity(rule 3):  compare(l, p1.l) + compare(p1.r, p2.l) + compare(r, p2.r)
    congruence  (rule 4):  per premise i in 0..k-1:
                           1 + compare(l.child_i, p_i.l) + compare(r.child_i, p_i.r)
    unfolding   (rule 5):  clause ordinal (select walk) + template_count + 1
                           (provision) + substitution traversal of the clause
                           body template against the right term
- derivation_conclude: compare(owner.left, last.l) + compare(owner.right, last.r)

Ground comparison (compare_ground_terms): +1 per visit, +1 per resume;
identical-ref pairs and session-memo hits resume without descending; the
session memo is shared across the whole request. Structural equality is
required by the caller, so only the equal path is modeled.

Substitution traversal: +1 per template-row visit, +1 per resume; variable
templates charge one ground compare of binding vs. right (session memo);
constructor templates verify right's tag/symbol then visit child pairs.
The substitution memo is local to each unfolding.

Run standalone to validate the model against every pinned encoder WORK
figure: it rebuilds each batch's derivation with the checked-in stepper
and compares the simulated cumulative work to `encoder.WORK`.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))


def simulate_work(theory, terms, owner_left, owner_right, proofs):
    """terms: ref -> (tag, sym, *children).  proofs: [(rule, l, r, *fields)].

    Returns the exact checker work figure for the request.
    """
    gmemo = set()

    def gcmp(a, b):
        used = 0
        stack = []
        cur = (a, b)
        while True:
            used += 1
            l, r = cur
            if l != r and (l, r) not in gmemo:
                tl, tr = terms[l], terms[r]
                assert tl[0] == tr[0] and tl[1] == tr[1], "unequal compare"
                pairs = list(zip(tl[3:], tr[3:]))
                if pairs:
                    stack.append([(l, r), pairs[1:]])
                    cur = pairs[0]
                    continue
                gmemo.add((l, r))
            # resume loop
            while True:
                used += 1
                if not stack:
                    return used
                key, sibs = stack[-1]
                if sibs:
                    cur = sibs[0]
                    stack[-1][1] = sibs[1:]
                    break
                stack.pop()
                gmemo.add(key)

    def unfold(l, r, ordinal):
        fn_sym = terms[l][1]
        _, args, mode, selected, clauses = theory.function(fn_sym)
        ctor, templates, body = clauses[ordinal - 1]
        charge = ordinal + len(templates) + 1
        # environment
        lt = terms[l]
        call_args = lt[3:]
        env = {}
        if mode == 0:
            for i, arg in enumerate(call_args):
                env[i] = arg
        else:
            for i, arg in enumerate(call_args):
                if i != selected:
                    env[i] = arg
            matched = terms[call_args[selected]]
            assert matched[0] == 1 and matched[1] == ctor
            for j, child in enumerate(matched[3:]):
                env[len(call_args) + j] = child
        smemo = set()

        def visit(row_idx, right_ref, frames):
            nonlocal charge
            charge += 1
            key = (row_idx, right_ref)
            if key in smemo:
                resume(frames)
                return
            rec = templates[row_idx - 1]
            if rec[0] == 0:
                charge += gcmp(env[rec[1]], right_ref)
                smemo.add(key)
                resume(frames)
            else:
                rt = terms[right_ref]
                assert rt[0] == rec[0] and rt[1] == rec[1], (
                    f"template/right head mismatch: {rec} vs {rt}")
                nxt = list(zip(rec[3:], rt[3:]))
                if nxt:
                    frames.append((key, nxt[1:]))
                    visit(nxt[0][0], nxt[0][1], frames)
                else:
                    smemo.add(key)
                    resume(frames)

        def resume(frames):
            nonlocal charge
            while True:
                charge += 1
                if not frames:
                    return
                key, sibs = frames[-1]
                if sibs:
                    row_idx, right_ref = sibs[0]
                    frames[-1] = (key, sibs[1:])
                    visit(row_idx, right_ref, frames)
                    return
                frames.pop()
                smemo.add(key)

        visit(body, r, [])
        return charge

    work = len(proofs) + 1
    for row in proofs:
        work += 1
        rule, l, r = row[0], row[1], row[2]
        if rule == 1:
            work += gcmp(l, r)
        elif rule == 2:
            p = proofs[row[3] - 1]
            work += gcmp(l, p[2]) + gcmp(r, p[1])
        elif rule == 3:
            p1, p2 = proofs[row[3] - 1], proofs[row[4] - 1]
            work += gcmp(l, p1[1]) + gcmp(p1[2], p2[1]) + gcmp(r, p2[2])
        elif rule == 4:
            k = row[3]
            lt, rt = terms[l], terms[r]
            for i in range(k):
                p = proofs[row[4 + i] - 1]
                work += 1 + gcmp(lt[3 + i], p[1]) + gcmp(rt[3 + i], p[2])
        else:
            work += unfold(l, r, row[3])
    work += gcmp(owner_left, proofs[-1][1]) + gcmp(owner_right, proofs[-1][2])
    return work


def main():
    import encoder
    import identity
    from stepper import Theory

    theory = Theory(identity.complete_theory())
    print(f"theory: {len(theory.functions)} functions, "
          f"{len(theory.constructors)} constructors")
    ok = True
    for name, fill in encoder.GROUPS:
        v = encoder.Vector(theory)
        fill(v)
        for left, _ in v.equations:
            v.s.prove(left)
        owner_terms, left_ref, right_ref, witness_terms, proofs = \
            v.s.encode(*v.equations[-1])
        terms = {i + 1: tuple(t) for i, t in enumerate(owner_terms)}
        base = len(owner_terms)
        for i, t in enumerate(witness_terms):
            terms[base + 1 + i] = tuple(t)
        work = simulate_work(theory, terms, left_ref, right_ref, proofs)
        pinned = encoder.WORK.get(name, 0)
        mark = "OK      " if work == pinned else "MISMATCH"
        if work != pinned:
            ok = False
        print(f"{mark} {name}: simulated={work} pinned={pinned} "
              f"rows={len(proofs)}")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
