"""fix3: validity by reading mutants; accepted_by; no diagnostic auto-confirm."""

from __future__ import annotations

import re
from dataclasses import dataclass

from mutation_validate import (
    REQUIRES_RE,
    LET_BIND_RE,
    binding_write_obligation,
    first_dropped_mut_binding,
    removed_requires_clauses,
    transition_restates_fact,
    SPEC_BY_KIND,
)

ROUTE_MEANS_INVALID = frozenset({"outside", "no-entry", "unreached", "missing"})


@dataclass
class Outcome:
    bucket: str
    accepted_by: str
    valid: bool
    obligation: str | None
    spec_path: str | None
    spec_quote: str | None
    spec_heading: str | None
    note: str


def checker_side(omg: str, cur: str) -> str:
    if omg == "accepted":
        return "accept"
    if omg == "skipped" and cur == "accepted":
        return "accept"
    if omg in ("timeout", "error") or cur in ("timeout", "error"):
        return "fail"
    if omg == "rejected" or cur == "rejected":
        return "reject"
    return "unknown"


def route_side(route: str) -> str:
    if route == "accepted":
        return "accept"
    if route in ("timeout", "error"):
        return "fail"
    if route in ("rejected", "incomplete", "fallback"):
        return "reject"
    return "unknown"


def accepted_by(omg: str, cur: str, route: str) -> str:
    cur_ok = checker_side(omg, cur) == "accept"
    route_ok = route_side(route) == "accept"
    if cur_ok and route_ok:
        return "both"
    if cur_ok:
        return "current"
    if route_ok:
        return "route"
    return ""


def disagree(omg: str, cur: str, route: str) -> bool:
    cs = checker_side(omg, cur)
    rs = route_side(route)
    if cs == "accept" and rs == "reject":
        return True
    if cs == "reject" and rs == "accept":
        return True
    if omg not in ("skipped", "") and omg in ("accepted", "rejected") and cur in ("accepted", "rejected") and omg != cur:
        return True
    return False


def _machine_name_before_requires(original: str, clause: str) -> str | None:
    marker = f"requires {clause}"
    idx = original.find(marker)
    if idx < 0:
        return None
    names = re.findall(r"machine\s+([\w:()<>, '\-]+)", original[:idx])
    return names[-1].strip() if names else None


def classify_drop_requires(original: str, mutant: str, omg: str, cur: str, route: str, route_reason: str) -> Outcome:
    from fix5_drop_requires import body_reestablishes_clause, fix5_spec_bundle
    removed = removed_requires_clauses(original, mutant)
    ab = accepted_by(omg, cur, route)
    cs, rs = checker_side(omg, cur), route_side(route)

    if not removed:
        return Outcome("rejected-both" if cs == "reject" and rs != "accept" else "discarded-valid", ab, True, None, None, None, None, "no removed requires")

    if cs == "reject" and rs != "accept":
        return Outcome("rejected-both", "", False, None, None, None, None, "")

    clause = removed[0]
    machine = _machine_name_before_requires(original, clause)
    path, heading, quote = fix5_spec_bundle()

    restated, req_q, trans_q = transition_restates_fact(mutant, clause)
    if restated:
        note = (
            f"valid: in-machine transition restates removed requires | "
            f"requires: `{req_q}` | transition: `{trans_q}`"
        )
        if ab:
            return Outcome("discarded-valid", ab, True, trans_q, None, None, None, note)
        return Outcome("discarded-valid", "", True, trans_q, None, None, None, note)

    if machine:
        in_body, req_q, rest_q = body_reestablishes_clause(mutant, machine, clause)
        if in_body:
            note = (
                f"valid: in-machine body/guard restates removed requires | "
                f"requires: `{req_q}` | restate: `{rest_q}`"
            )
            if ab:
                return Outcome("discarded-valid", ab, True, rest_q, None, None, None, note)
            return Outcome("discarded-valid", "", True, rest_q, None, None, None, note)

    # fix5: caller-established facts outside the machine do not heal modular entry.
    obl = f"removed requires `{clause}`"
    if machine:
        obl = f"machine `{machine}` missing entry `{clause}`"
    note = f"invalid (modular entry): {obl}"
    if ab:
        return Outcome("confirmed-hole", ab, False, obl, path, quote, heading, note)
    if disagree(omg, cur, route):
        return Outcome("disagreement-valid", accepted_by(omg, cur, route), False, obl, path, quote, heading, note)
    return Outcome("rejected-both", "", False, obl, None, None, None, note)


def _ref_diff_lines(original: str, mutant: str) -> list[tuple[str, str]]:
    out: list[tuple[str, str]] = []
    for o, m in zip(original.splitlines(), mutant.splitlines()):
        if o != m and ("&mut" in o or "&mut" in m or "& " in o):
            out.append((o.strip(), m.strip()))
    if len(original.splitlines()) != len(mutant.splitlines()):
        for o, m in zip(original.splitlines(), mutant.splitlines()):
            if "&mut" in o or "&mut" in m:
                out.append((o.strip(), m.strip()))
    return out


def classify_reference_kind(kind: str, original: str, mutant: str, omg: str, cur: str, route: str) -> Outcome:
    ab = accepted_by(omg, cur, route)
    cs, rs = checker_side(omg, cur), route_side(route)
    if cs == "reject" and rs != "accept":
        return Outcome("rejected-both", "", False, None, None, None, None, "")

    diffs = _ref_diff_lines(original, mutant)
    if not diffs:
        return Outcome("discarded-valid", ab, True, None, None, None, None, "no reference diff")

    _, mline = diffs[0]
    oline = diffs[0][0]

    # Q7: only invalidity is &mut of a let-bound name that lost mut.
    dropped = first_dropped_mut_binding(original, mutant)
    if dropped:
        obl = binding_write_obligation(dropped, mutant)
        if obl:
            path, heading, quote = SPEC_BY_KIND["remove_mut"]
            note = f"Q7: `let {dropped}` not mut; `{obl}`"
            if ab:
                return Outcome("known-gap-q7-mut", ab, False, obl, path, quote, heading, note)
            return Outcome("rejected-both", "", False, obl, None, None, None, note)

    # Shared-typed slot from mut reference: `let x: &T = &mut y`
    if re.search(r":\s*&[^m]", mline) and "&mut" in mline:
        obl = mline
        path, heading, quote = SPEC_BY_KIND["mut_ref_instead_of_shared"]
        invalid = "invalid: shared type receives `&mut` source"
        if ab:
            return Outcome("confirmed-hole", ab, False, obl, path, quote, heading, invalid)
        if disagree(omg, cur, route):
            return Outcome("disagreement-valid", accepted_by(omg, cur, route), False, obl, path, quote, heading, invalid)
        return Outcome("rejected-both", "", False, obl, None, None, None, invalid)

    # Parameter or field: &mut added where & was — invalid only if callee/caller writes through it.
    if kind == "shared_instead_of_mut":
        pm = re.search(r"(\w+)\s*:\s*&\s", mline)
        if pm:
            pname = pm.group(1)
            writes = [
                ln.strip()
                for ln in mutant.splitlines()
                if pname in ln and "=" in ln and not ln.strip().startswith("//")
            ]
            if not writes:
                note = f"valid: `{pname}` still shared; no write through parameter"
                if ab:
                    return Outcome("discarded-valid", ab, True, mline, None, None, None, note)
                return Outcome("discarded-valid", "", True, mline, None, None, None, note)
            obl = writes[0]
            path, heading, quote = SPEC_BY_KIND["shared_instead_of_mut"]
            invalid = f"invalid: write through shared borrow of `{pname}`"
            if ab:
                return Outcome("confirmed-hole", ab, False, obl, path, quote, heading, invalid)
            return Outcome("disagreement-valid", accepted_by(omg, cur, route), False, obl, path, quote, heading, invalid)

    if kind == "mut_ref_instead_of_shared":
        # &mut parameter where & was: invalid if body assigns through parameter.
        pm = re.search(r"(\w+)\s*:\s*&mut", mline)
        writes: list[str] = []
        if pm:
            pname = pm.group(1)
            # find machine body
            idx = mutant.find(mline)
            body = mutant[idx:] if idx >= 0 else mutant
            writes = [
                ln.strip()
                for ln in body.splitlines()[:80]
                if re.search(rf"\b{re.escape(pname)}\b", ln)
                and ("=" in ln or f"&mut {pname}" in ln)
                and not ln.strip().startswith("//")
                and "machine " not in ln
            ]
            if not writes and pname not in oline:
                note = f"valid: `&mut {pname}` in signature only (no callee write)"
                if ab:
                    return Outcome("discarded-valid", ab, True, mline, None, None, None, note)
                return Outcome("discarded-valid", "", True, mline, None, None, None, note)
            obl = writes[0] if writes else mline
        else:
            obl = mline

        # &mut local loan without mut binding on same name — Q7 if pattern `&mut name` with `let name`
        lm = re.search(r"&mut\s+(\w+)", obl)
        if lm:
            name = lm.group(1)
            letm = re.search(rf"\blet\s+{name}\b", mutant)
            let_mut = re.search(rf"\blet\s+mut\s+{name}\b", mutant)
            if letm and not let_mut:
                path, heading, quote = SPEC_BY_KIND["remove_mut"]
                note = f"Q7: non-mut `let {name}` with `{obl}`"
                if ab:
                    return Outcome("known-gap-q7-mut", ab, False, obl, path, quote, heading, note)

        if obl.startswith("//"):
            return Outcome("discarded-valid", ab, True, mline, None, None, None, "reference change in comment only")

        path, heading, quote = SPEC_BY_KIND["mut_ref_instead_of_shared"]
        if re.search(r":\s*&[^m]", obl) and "&mut" in obl:
            invalid = "invalid: shared-typed slot from `&mut` source"
        elif pm and writes:
            invalid = "invalid: callee writes through `&mut` parameter"
        else:
            note = "valid: `&mut` added without conflicting write or type mismatch (hand)"
            if ab:
                return Outcome("discarded-valid", ab, True, obl, None, None, None, note)
            return Outcome("discarded-valid", "", True, obl, None, None, None, note)
        if ab:
            return Outcome("confirmed-hole", ab, False, obl, path, quote, heading, invalid)
        if disagree(omg, cur, route):
            return Outcome("disagreement-valid", accepted_by(omg, cur, route), False, obl, path, quote, heading, invalid)
        return Outcome("rejected-both", "", False, obl, None, None, None, invalid)

    return Outcome("discarded-valid", ab, True, mline, None, None, None, "unclassified reference mutation")


def classify_fix3(
    kind: str,
    original: str,
    mutant: str,
    omg: str,
    cur: str,
    route: str,
    route_reason: str,
) -> Outcome:
    from fix4_apply import apply_fix4
    oc: Outcome
    if omg == "timeout" or cur == "timeout" or route == "timeout":
        oc = Outcome("timeout", "", False, None, None, None, None, "timeout")
    elif omg == "error" or route == "error":
        oc = Outcome("failed-run", "", False, None, None, None, None, route_reason[:200])
    elif kind == "drop_requires":
        oc = classify_drop_requires(original, mutant, omg, cur, route, route_reason)
    elif kind in ("shared_instead_of_mut", "mut_ref_instead_of_shared"):
        oc = classify_reference_kind(kind, original, mutant, omg, cur, route)
    elif kind in ("drop_mut", "remove_mut"):
        from mutation_validate import classify_remove_mut

        if checker_side(omg, cur) != "accept" and route_side(route) != "accept":
            oc = Outcome("rejected-both", "", False, None, None, None, None, "")
        else:
            v = classify_remove_mut(original, mutant, omg, cur, route)
            ab = accepted_by(omg, cur, route)
            oc = Outcome(
                v.bucket,
                ab if v.bucket in ("known-gap-q7-mut", "confirmed-hole") else "",
                False,
                v.obligation_line,
                v.spec_path,
                v.spec_quote,
                v.spec_heading,
                v.notes,
            )
    else:
        ab = accepted_by(omg, cur, route)
        cs, rs = checker_side(omg, cur), route_side(route)
        if cs == "reject" and rs != "accept":
            oc = Outcome("rejected-both", "", False, None, None, None, None, "")
        elif kind == "drop_ensures":
            oc = (
                Outcome("discarded-valid", ab, True, None, None, None, None, "ensures removal weakens only")
                if ab
                else Outcome("rejected-both", "", True, None, None, None, None, "")
            )
        elif kind in ("drop_guard", "break_ensures", "widen_requires"):
            oc = (
                Outcome("discarded-valid", ab, True, None, None, None, None, "not auto-confirmed (fix3)")
                if ab
                else Outcome("rejected-both", "", False, None, None, None, None, "")
            )
        elif ab and kind:
            oc = Outcome("discarded-valid", ab, True, None, None, None, None, "unsettled; default valid")
        else:
            oc = Outcome("rejected-both", "", False, None, None, None, None, "")

    return apply_fix4(oc, kind, original, mutant, omg, cur, route)
