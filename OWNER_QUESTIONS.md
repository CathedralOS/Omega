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
   > Owner: This is specifically about getting a data ref on which we can call methods? It needs some form of dependency injection then (similar concept to SAS-components in Cathedral -- the main function literally is given the filesystem instance, although build.omg still needs to include std::filesystem to use it).
3. **Grant derivation defaults.** Read root = the package dir (main.omg's
   directory)? Write root = which output dir? May build.omg request EXTRA
   roots (assets outside the tree), and does that require CLI
   acknowledgment (`--allow-read=...`)?
   > Owner: Not sure what you mean. Presumably build.omg is within the dir being built, and it builds to build/. In some sense, main.omg is NOT a blessed name, build.omg should probably specify it, with the exception that we <may> decide to support a "default build.omg" if trying to build some non-build.omg file. I don't think we give a shit about permissions at this point, if you are over-indexing on Cathedral-like permissions & granting.
4. **Effect-gate shape.** Relax build_config.rs's empty-effect gate to
   "transitive effects ⊆ {filesystem}" unconditionally, or behind an
   explicit opt-in (in build.omg or on the CLI)?
   > Owner: What the fuck is build_config.rs? build.omg may effect filesystem, if thats related. It should be a declared effect on the main func within build (and forbidden otherwise, naturally, by our effect system & trying to call filesystem funcs). Is this what you are trying to say?
5. **Console for build logging.** The granted entry currently rejects
   Console strictly (only Filesystem granted). Print-logging is the
   obvious want; stdout is interpreter-captured anyway. Grant it?
   (Strict = the reversible choice, so strict ships until answered.)
   > Owner: Add this to build.omg effects too, its harmless and everyone wants it. Interpreter should never "just catch it" if build.omg is logging, in my mind this is a declared effect and thus should be treated seriously.

## Recursion directive scope (main lane's review items + one fs-found gap)

5. **Bare `-> own_entry(..)` loop-back** (their review item 1): acceptable
   as the blessed loop spelling, or must loops be spelled through explicit
   sub-states only? Your countdown comment ("removing the `self` keyword
   doesn't change fuck-all") reads as rejecting even the bare spelling —
   but the enforced error + the corpus sweep + runtime_loop_* canaries
   currently BLESS bare loop-backs. Confirm or extend.
   [Main lane, so the cost is on the table: a "banned" answer tears out
   ~12 pass canaries pinning the spelling (the termination/measure family
   + recursive-walk + loop_{accumulator,rotation} + the just-landed
   bind-first serve), the entry-reentry `decreases` proof surface (its
   only current consumer), the recursive-clone specialization, and the
   unserved-recursive-result sweep, plus corpus rewrites (proofs, std fs
   mkall, dungeon parser) into explicit sub-state self-transition loops.
   Pre-scoped in TASKS.md; teardown starts on the answer. We read your
   comment as YES-banned and are holding only for this confirmation.]
> I dont get why you are so retarded on recursion. machine call cycles = banned. its that fucking simple. Everything that hinges on the contrary is invalid Omega. 'decreases' stuff is for states. States are not recursion. They are transitions, jumps, goto, whatever. Thus this is equal to a for loop, or a while loop. I dont understand why you cant grasp this? Am I missing nuance?
   [No missing nuance -- your distinction IS the implementation: machine
   CALLS are stack-based; `-> target(args)` arms are TRANSITIONS (jumps
   with re-bound args, constant stack), including when the target is the
   machine's own entry. So: bare loop-backs and states-scoped `decreases`
   stay (they are for-loops under your ruling); the enforced bans become
   the CALL-graph ones -- Q6 mutual value-call cycles and Q7 statement
   tail self-calls, both now filed as engineering in TASKS.md. The earlier
   countdown confusion was ours: we read your note as banning the
   transition spelling too.]
6. **Mutual value-call cycles** (their review item 2): `A calls B calls A`
   (the dungeon's find_item_at/find_item_after pair) still compiles — the
   cycle check does not see value calls. Kill (needs the value-call cycle
   walk) or keep (bounded clone specialization absorbs them)?
> Owner: yes fucking banned.
7. **Statement-position self-call** (fs-found while retracting): a
   TRAILING statement `self.drip(n - 1);` still compiles+runs — it lowers
   as a Nested-transition loop upstream of the call plan (mechanically a
   loop, spelled as a call). The corpus sweep rewrote these spellings but
   the route still accepts them. In or out?
   > Are you asking about "lowering a tail call to a loop"? Banned, if it reads as recursion. We can maybe relax this later, but as of now, go write this as states.

## Underspecified numerics (main lane's found items, both marked "owner call")

8. **Float domain clauses.** `f: f32 in Saturating` compiles but means
   nothing (both engines run plain IEEE; overflow → inf). Reject domains
   on float primitives loudly (matches decision-17's integer framing), or
   define float saturation (clamp to finite MAX)?
   > Not defined yet. Deferred pending a serious float design.
9. **Range constraint under a non-Exact domain.** `i: usize [0..=4] in
   Wrapping` accepts `self.i = 100` — the range only enforces under Exact,
   so the declaration lies. Ill-formed (reject the combination at
   declaration), or define stores to wrap/clamp INTO the declared range?
   > Oh I misread this in the other doc. Well, this in my mind is a compile error. You can surface this as a "Exact assignments must be within invariant range, consider adjusting the size or using a modulo operator" or whatever the fuck.

## Host bindings (fs lane, flagged during provides work)

10. **Interpreter story for authored import bindings.** A user-authored
    import row (`beep -> DllImport("msvcrt.dll","abs")`) is NATIVE-ONLY;
    the interpreter declines and the differential skips. Should
    interpreted runs (a) always decline authored imports (differential
    stays skip), (b) get a virtual-stub mechanism (author declares the
    virtual semantics next to the row), or (c) something capability-gated
    like the build.omg fs grants? Affects how testable authored-binding
    programs are.

11. **A std console boundary for build.omg logging (fs lane, 2026-07-11k).**
    Owner answer #5 landed: granted builds SERVE console writes through
    DECLARED `stdout_io`/`stderr_io` rows, flushed to the compiler's real
    streams. Today every build.omg spells its own boundary
    (`boundary trait BuildLog { machine write_line(text: &[u8]) effects
    stdout_io; }` -- the fail canary teaches this). Should std ship a
    canonical console/log boundary with declared rows (name? `Console`
    collides with the bare exit_process convention all samples use;
    `BuildLog`? method set: write/write_line/write_error/write_error_line?),
    or is per-program spelling the intended shape?

12. **Byte-level stdin spelling for the Omega frontend (fs lane, 2026-07-11w).**
    The stdin samples (stdin_checksum/rot1/upper — the samples_compile
    baseline's last three reds) spell `read_byte()`/`write_byte(b)` as FREE
    functions: that is the BOOTSTRAP lattice's input model (omega2gamma's
    threaded stdin-list + accumulated stdout), not a settled Omega surface.
    The real frontend has `Console.read_line` (boundary, line-oriented) only.
    Should Omega grow (a) byte ops on the Console boundary
    (`read_byte() -> i32` EOF=-1, `write_byte(b)` — with effect rows), (b) a
    separate Stdin/Stdout boundary pair, or (c) keep byte I/O
    lattice-only and rewrite the samples line-oriented? The samples stay
    red on the Rust compiler until ruled.
