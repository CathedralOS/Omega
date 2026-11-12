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
    discard_class: str = ""
    not_q7_reason: str = ""
    runtime_trap_scan: str = ""


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
        dc = "drop-req-transition-restate"
        if ab:
            return Outcome("discarded-valid", ab, True, trans_q, None, None, None, note, dc)
        return Outcome("discarded-valid", "", True, trans_q, None, None, None, note, dc)

    if machine:
        in_body, req_q, rest_q = body_reestablishes_clause(mutant, machine, clause)
        if in_body:
            dc = "drop-req-guard-restate" if "access:" in rest_q else "drop-req-body-restate"
            note = (
                f"valid: in-machine body/guard restates removed requires | "
                f"requires: `{req_q}` | restate: `{rest_q}`"
            )
            if ab:
                return Outcome("discarded-valid", ab, True, rest_q, None, None, None, note, dc)
            return Outcome("discarded-valid", "", True, rest_q, None, None, None, note, dc)

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


_WRITE_SKIP = (
    "requires ",
    "ensures ",
    "let ",
    "const ",
    "data ",
    "machine ",
    "public ",
    "transition",
    "//",
    "import ",
    "terminates ",
)


def _demoted_mut_binding(mline: str) -> str | None:
    pm = re.search(r"(\w+)\s*:\s*&\s*[\w:<\['\"]", mline)
    if pm:
        return pm.group(1)
    return None


def _checktime_write_via_name(mutant: str, name: str) -> str | None:
    for ln in mutant.splitlines():
        s = ln.strip()
        if any(s.startswith(p) for p in _WRITE_SKIP):
            continue
        if re.search(rf"\b{re.escape(name)}\s*\.[A-Za-z0-9_\[\]]+\s*=", s):
            return s
        if re.search(rf"\b{re.escape(name)}\s*=", s) and "==" not in s:
            return s
    return None


def _shared_slot_from_mut_source(mline: str) -> bool:
    return bool(re.search(r":\s*&[^m]", mline) and "&mut" in mline)


def _loan_root(expr: str) -> str:
    expr = expr.strip().rstrip(";")
    head = re.split(r"\[", expr, maxsplit=1)[0]
    return head.split(".")[0].strip()


def _place_is_mut_at_check(mutant: str, loan_expr: str) -> tuple[bool, str]:
    root = _loan_root(loan_expr)
    if re.search(rf"\blet\s+mut\s+{re.escape(root)}\b", mutant):
        return True, f"source `let mut {root}`"
    if re.search(rf"\(\s*[^)]*\b{re.escape(root)}\s*:\s*&mut\b", mutant):
        return True, f"source parameter `{root}: &mut _`"
    if re.search(rf"\b{re.escape(root)}\s*:\s*&mut\b", mutant):
        return True, f"source binding `{root}: &mut _`"
    return False, f"source `let {root}` without `mut`"


def _callee_param_write(mutant: str, sig_hint: str, pname: str) -> str | None:
    idx = mutant.find(sig_hint)
    chunk = mutant[idx : idx + 2500] if idx >= 0 else mutant
    started = False
    for ln in chunk.splitlines():
        if not started and pname in ln and ("machine " in ln or f"({pname}" in ln):
            if "{" in ln:
                started = True
            continue
        if not started:
            continue
        s = ln.strip()
        if s.startswith("requires ") or s.startswith("machine ") or s.startswith("transition"):
            continue
        if re.search(rf"\b{re.escape(pname)}\b", s) and "=" in s and "==" not in s:
            if not s.startswith("let "):
                return s
    return None


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

    path_sm, heading_sm, quote_sm = SPEC_BY_KIND["shared_instead_of_mut"]
    path_mr, heading_mr, quote_mr = SPEC_BY_KIND["mut_ref_instead_of_shared"]

    if kind == "shared_instead_of_mut":
        pname = _demoted_mut_binding(mline)
        obl = None
        if pname:
            obl = _checktime_write_via_name(mutant, pname) or _callee_param_write(mutant, mline, pname)
        if _shared_slot_from_mut_source(mline):
            obl = mline
        if not obl:
            note = "valid: demoted `&mut`→`&` without check-time write through shared place"
            dc = "ref-signature-or-call-only"
            if ab:
                return Outcome(
                    "discarded-valid", ab, True, mline, None, None, None, note, dc,
                    "not Q7: mutation is reference qualification, not `remove_mut`",
                )
            return Outcome("discarded-valid", "", True, mline, None, None, None, note, dc)
        path, heading, quote = path_sm, heading_sm, quote_sm
        invalid = f"invalid: check-time write or `&mut` loan through shared place `{obl}`"
        nq = "not Q7: no dropped `let mut` binding; obligation is shared/mut reference rule"
        if ab:
            return Outcome("confirmed-hole", ab, False, obl, path, quote, heading, invalid, "", nq)
        if disagree(omg, cur, route):
            return Outcome(
                "disagreement-valid", accepted_by(omg, cur, route), False, obl, path, quote, heading, invalid, "", nq
            )
        return Outcome("rejected-both", "", False, obl, None, None, None, invalid)

    if kind == "mut_ref_instead_of_shared":
        if _shared_slot_from_mut_source(mline):
            obl = mline
            src_m = re.search(r"&mut\s+(.+)$", mline)
            src_expr = src_m.group(1).strip() if src_m else ""
            is_mut, src_reason = _place_is_mut_at_check(mutant, src_expr)
            if is_mut:
                note = f"valid: `&mut` loan into shared slot is reborrow from mut place ({src_reason})"
                if ab:
                    return Outcome(
                        "discarded-valid",
                        ab,
                        True,
                        obl,
                        None,
                        None,
                        None,
                        note,
                        "mut-ref-reborrow-from-mut-place",
                    )
                return Outcome(
                    "discarded-valid", "", True, obl, None, None, None, note, "mut-ref-reborrow-from-mut-place"
                )
            path, heading, quote = SPEC_BY_KIND["remove_mut"]
            note = f"Q7: {src_reason}; `{obl}`"
            if ab:
                return Outcome("known-gap-q7-mut", ab, False, obl, path, quote, heading, note)
            return Outcome("rejected-both", "", False, obl, None, None, None, note)

        pm = re.search(r"(\w+)\s*:\s*&mut", mline)
        callee_write = _callee_param_write(mutant, mline, pm.group(1)) if pm else None
        if pm and callee_write:
            obl = callee_write
            invalid = f"invalid: callee writes through `&mut` parameter `{pm.group(1)}`"
            if ab:
                return Outcome("confirmed-hole", ab, False, obl, path_mr, quote_mr, heading_mr, invalid)
            if disagree(omg, cur, route):
                return Outcome(
                    "disagreement-valid", accepted_by(omg, cur, route), False, obl, path_mr, quote_mr, heading_mr, invalid
                )
            return Outcome("rejected-both", "", False, obl, None, None, None, invalid)

        obl = mline
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
                return Outcome("rejected-both", "", False, obl, None, None, None, note)

        note = "valid: `&mut` at least as strict as `&` without shared-slot violation"
        dc = "mut-ref-stricter-signature"
        if ab:
            return Outcome("discarded-valid", ab, True, mline, None, None, None, note, dc)
        return Outcome("discarded-valid", "", True, mline, None, None, None, note, dc)

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
                Outcome(
                    "discarded-valid", ab, True, None, None, None, None, "ensures removal weakens only", "ensures-weaken"
                )
                if ab
                else Outcome("rejected-both", "", True, None, None, None, None, "")
            )
        elif kind in ("drop_guard", "break_ensures", "widen_requires"):
            oc = (
                Outcome(
                    "discarded-valid",
                    ab,
                    True,
                    None,
                    None,
                    None,
                    None,
                    "not auto-confirmed (fix3)",
                    f"accepted-{kind}",
                )
                if ab
                else Outcome("rejected-both", "", False, None, None, None, None, "")
            )
        elif ab and kind:
            oc = Outcome("discarded-valid", ab, True, None, None, None, None, "unsettled; default valid", "other-accepted")
        else:
            oc = Outcome("rejected-both", "", False, None, None, None, None, "")

    return apply_fix4(oc, kind, original, mutant, omg, cur, route)
