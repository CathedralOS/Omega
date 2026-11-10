# sites_001

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| F0001 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:704 | SOUND | USE aborts integer formation when the flag is set; no goal is marked proven. |
| F0002 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:835 | SOUND | USE drops formation metadata only; rank callers treat missing formation as failure, not proof. |
| F0003 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:1788 | SOUND | USE re-closes the bound matrix when the flag is already set; sets are conservative contradiction tests. |

## F0001

**Role:** USE (`read`, `contract_entailment.rs:704`).

**Enclosing function:** `Engine::form_integer_expression` (`read`, `contract_entailment.rs:682–717`). After cloning `self` into `admitted`, running `formed_value` on the body, and optionally checking `formed_type` against the declared result type, the function returns `None` when `admitted.requires_unsatisfiable` is true (`read`, `contract_entailment.rs:702–706`).

**What the flag means here:** The flag marks the arithmetic engine’s hypotheses as unsatisfiable (`read`, INSTRUCTIONS.md STREAM F). At this site it does not turn any goal into proven; it rejects formation entirely.

**Upward effect:** Callers such as `scoped_integer_expression_formation` already bail out if `engine.requires_unsatisfiable` before calling (`read`, `contract_entailment.rs:37184–37191`). If this line fires, `form_integer_expression` returns `None`, so no `IntegerApplicationFormation` or scoped captured value is produced (`read`, `contract_entailment.rs:37190–37205`). That withholds integer-application evidence rather than proving contracts.

**Where the flag is set (relevant to soundness):** On the `admitted` engine, typical setters include `install_hypotheses` (constant lower bound strictly below a required bound at `contract_entailment.rs:6707–6709`; `NotEqual` when both a difference and its negation are proved nonnegative at `contract_entailment.rs:6724–6733`; `read`), `close_matrix` OR-ing `matrix.close` (`read`, `contract_entailment.rs:1830–1831`; closure detects a positive lower bound on a zero-length difference cycle at `arithmetic_domains.rs:1576–1579`), and boolean guard contradictions in `collect_condition_comparisons` (`read`, `contract_entailment.rs:5434–5436`). Those tests encode genuine contradictions when they fire (`read`).

**Verdict:** **SOUND** — whenever the flag is set on this path, the hypothesis set is inconsistent under the checks above; this USE only fails closed and does not wrongly prove obligations.

## F0002

**Role:** USE (`read`, `contract_entailment.rs:835`).

**Enclosing function:** `Engine::form_declared_integer_application` (`read`, `contract_entailment.rs:719–844`). After forming the measure body under formal arguments, pushing declared min/max bounds, and calling `refresh_difference_bounds` (`read`, `contract_entailment.rs:822–833`), formation metadata is `None` when `admitted.requires_unsatisfiable` is true, but `FormedIntegerApplication { value, formation }` is still returned with the computed polynomial (`read`, `contract_entailment.rs:834–843`).

**What becomes proven / skipped:** Dropping `formation` does not mark ranking or entailment goals proven. Downstream, `RankValue::formation` calls this function then `install_hypotheses` with an equality between the stored rank value and `formed.value`, and returns `formed.formation` (`read`, `contract_entailment.rs:32141–32149`). `RankValue::forms` is true only when `formation(...).is_some()` (`read`, `contract_entailment.rs:32130–32131`), so a absent formation blocks rank-evidence installation rather than vacuously proving decrease.

**Upward effect:** Lets the engine retain a normalized polynomial for diagnostics or later steps while refusing to record casts/calls/dispatches decisions when hypotheses are inconsistent (`read`, `contract_entailment.rs:834–841`). Scoped wrappers require `application.formation?` (`read`, `contract_entailment.rs:37167`, `37205`), so unsatisfiable hypotheses still fail the scoped API.

**Verdict:** **SOUND** — flag true implies contradictory bounds installed before line 835 (`read`, bounds push and `refresh_difference_bounds` at `822–833`); clearing formation prevents treating the application as well-formed evidence, and callers do not convert that into proven goals.

## F0003

**Role:** USE (`read`, `contract_entailment.rs:1788`).

**Enclosing function:** `Engine::refresh_difference_bounds` (`read`, `contract_entailment.rs:1787–1790`). When `seed_matrix()` records new difference seeds or `self.requires_unsatisfiable` is already true, the engine calls `close_matrix()` (`read`, `contract_entailment.rs:1788–1789`).

**What the flag does here:** It does not directly prove goals. It forces another Floyd–Warshall-style closure on `difference_matrix::DifferenceBounds` (`read`, `contract_entailment.rs:1830–1831`, `arithmetic_domains.rs:1517–1562`). `close_matrix` may set the flag via `self.requires_unsatisfiable |= self.matrix.close(ZERO_ATOM)` (`read`, `contract_entailment.rs:1831`).

**Why re-close when already unsatisfiable:** Once hypotheses are known inconsistent, propagating tightened difference bounds cannot restore satisfiability; closure only strengthens detection of positive self-loop bounds (`read`, `arithmetic_domains.rs:1576–1579`). Upstream, `install_hypotheses` invokes `refresh_difference_bounds` after installing comparisons (`read`, `contract_entailment.rs:6716`, `6739–6740`), so this site sits on the path that feeds `prove_at_least` / `integer_comparisons_unsatisfiable` (`read`, `contract_entailment.rs:6726–6737`, `6745–6776`).

**Verdict:** **SOUND** — the flag at this USE reflects prior conservative contradiction detection; using it to close the matrix does not mark obligations proven and does not invent contradictions beyond `matrix.close`’s positive-cycle test.

## What I did not read

- Full bodies of `dispatch::form`, `calls::form`, and every caller of `form_declared_integer_application` outside the rank/scoped paths cited above (`inferred` partial coverage only).
- Entire `LowerBoundQueries` / Horner and conditional-bound implementation beyond `prove_at_least` and `prove_base_lower_bound` prefixes (`read`, `contract_entailment.rs:3378–3492`, `1719–1720`).
- `hypotheses_contradictory` arm-judge sites (separate identifier from this TSV).
- `git show 457adf4a11f:...` for digest line parity (current checkout lines match search snapshot at `704`, `835`, `1788`; no re-location needed).

SITES COMPLETE
