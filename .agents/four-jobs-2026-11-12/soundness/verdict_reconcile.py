"""Map checker verdict pairs to review buckets (fix3 verdict policy)."""

from __future__ import annotations

from dataclasses import replace

from fix3_classify import Outcome, accepted_by, both_checkers_rejected, omega_current_disagree
from fix4_apply import route_is_outside


def any_checker_accepts(omg: str, cur: str) -> bool:
    return omg == "accepted" or cur == "accepted"


def reconcile_verdict_bucket(
    oc: Outcome,
    kind: str,
    omg: str,
    cur: str,
    route: str,
) -> Outcome:
    """Enforce bucket rules after semantic classification."""
    if kind == "no-mutation" or oc.bucket in ("no-mutation", "timeout", "failed-run"):
        return oc

    ab = accepted_by(omg, cur, route)

    if both_checkers_rejected(omg, cur):
        if oc.bucket != "rejected-both":
            return replace(
                oc,
                bucket="rejected-both",
                accepted_by="",
                valid=False,
                note=(oc.note + " | verdict: both checkers rejected").strip(" |"),
            )
        return oc

    if omega_current_disagree(omg, cur):
        if oc.valid:
            side = "current" if cur == "accepted" else "omega"
            return replace(
                oc,
                bucket="disagreement-valid",
                accepted_by=side,
                valid=True,
                note=(oc.note + f" | verdict: valid mutant, {side} accepts").strip(" |"),
            )
        if oc.bucket in ("confirmed-hole", "known-gap-q7-mut"):
            return replace(oc, accepted_by=ab or oc.accepted_by, valid=False)
        if oc.bucket in ("valid-route-outside", "discarded-valid"):
            # Invalid but one checker accepts: should have been q7/confirmed already
            if "Q7:" in oc.note or oc.bucket == "known-gap-q7-mut":
                return replace(oc, bucket="known-gap-q7-mut", accepted_by=ab, valid=False)
        return oc

    if any_checker_accepts(omg, cur):
        if route_is_outside(route):
            if oc.valid:
                if oc.bucket != "valid-route-outside":
                    return replace(
                        oc,
                        bucket="valid-route-outside",
                        accepted_by="current" if cur == "accepted" else ab,
                        valid=True,
                        note=(oc.note + f" | verdict: valid, route `{route}` outside").strip(" |"),
                    )
                return oc
            # Invalid mutant accepted by checker — never valid-route-outside
            if oc.bucket == "valid-route-outside":
                if "Q7:" in oc.note:
                    return replace(
                        oc,
                        bucket="known-gap-q7-mut",
                        accepted_by=ab,
                        valid=False,
                        note=oc.note.replace("valid mutant; route", "invalid (Q7); route"),
                    )
                if oc.spec_path:
                    return replace(
                        oc,
                        bucket="confirmed-hole",
                        accepted_by=ab,
                        valid=False,
                        note=(oc.note + " | verdict: invalid accepted despite route outside").strip(" |"),
                    )
                return replace(
                    oc,
                    bucket="discarded-valid",
                    accepted_by=ab,
                    valid=True,
                    discard_class=oc.discard_class or "invalid-accepted-downgrade",
                    note=(oc.note + " | verdict: could not promote invalid accept").strip(" |"),
                )
        if oc.bucket == "disagreement-valid" and not oc.valid:
            return replace(
                oc,
                bucket="confirmed-hole" if oc.spec_path else "known-gap-q7-mut",
                accepted_by=ab,
                valid=False,
            )
    return oc
