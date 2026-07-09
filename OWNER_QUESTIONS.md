# Owner Questions — consolidated digest (2026-07-10l, fs lane)

Every decision currently WAITING ON ZACH, gathered from all three task
files so they can be batch-answered. Each names what it unblocks. Answers
can go inline here, in chat, or in the source task file — the lanes sync
from all of them.

## build.omg (fs lane; blocks the final compiler-side rung of open-work #3)

1. **Capability injection spelling.** How does `machine build(b: &mut
   Build)` receive its `Filesystem`? Options: a field on Build (`b.fs`), a
   second parameter (`build(b, fs: &mut Filesystem)`), or machine-owned
   data. Every build.omg will spell this. (Interpreter side is landed and
   parameter-driven; only the spelling is open.)
2. **Grant derivation defaults.** Read root = the package dir (main.omg's
   directory)? Write root = which output dir? May build.omg request EXTRA
   roots (assets outside the tree), and does that require CLI
   acknowledgment (`--allow-read=...`)?
3. **Effect-gate shape.** Relax build_config.rs's empty-effect gate to
   "transitive effects ⊆ {filesystem}" unconditionally, or behind an
   explicit opt-in (in build.omg or on the CLI)?
4. **Console for build logging.** The granted entry currently rejects
   Console strictly (only Filesystem granted). Print-logging is the
   obvious want; stdout is interpreter-captured anyway. Grant it?
   (Strict = the reversible choice, so strict ships until answered.)

## Recursion directive scope (main lane's review items + one fs-found gap)

5. **Bare `-> own_entry(..)` loop-back** (their review item 1): acceptable
   as the blessed loop spelling, or must loops be spelled through explicit
   sub-states only? Your countdown comment ("removing the `self` keyword
   doesn't change fuck-all") reads as rejecting even the bare spelling —
   but the enforced error + the corpus sweep + runtime_loop_* canaries
   currently BLESS bare loop-backs. Confirm or extend.
6. **Mutual value-call cycles** (their review item 2): `A calls B calls A`
   (the dungeon's find_item_at/find_item_after pair) still compiles — the
   cycle check does not see value calls. Kill (needs the value-call cycle
   walk) or keep (bounded clone specialization absorbs them)?
7. **Statement-position self-call** (fs-found while retracting): a
   TRAILING statement `self.drip(n - 1);` still compiles+runs — it lowers
   as a Nested-transition loop upstream of the call plan (mechanically a
   loop, spelled as a call). The corpus sweep rewrote these spellings but
   the route still accepts them. In or out?

## Underspecified numerics (main lane's found items, both marked "owner call")

8. **Float domain clauses.** `f: f32 in Saturating` compiles but means
   nothing (both engines run plain IEEE; overflow → inf). Reject domains
   on float primitives loudly (matches decision-17's integer framing), or
   define float saturation (clamp to finite MAX)?
9. **Range constraint under a non-Exact domain.** `i: usize [0..=4] in
   Wrapping` accepts `self.i = 100` — the range only enforces under Exact,
   so the declaration lies. Ill-formed (reject the combination at
   declaration), or define stores to wrap/clamp INTO the declared range?

## Host bindings (fs lane, flagged during provides work)

10. **Interpreter story for authored import bindings.** A user-authored
    import row (`beep -> DllImport("msvcrt.dll","abs")`) is NATIVE-ONLY;
    the interpreter declines and the differential skips. Should
    interpreted runs (a) always decline authored imports (differential
    stays skip), (b) get a virtual-stub mechanism (author declares the
    virtual semantics next to the row), or (c) something capability-gated
    like the build.omg fs grants? Affects how testable authored-binding
    programs are.
