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
    > Owner: Is this a windows only question? cross-platform? In my mind,
    > all libraries in the like must go behind a boundary declaration,
    > this is where trust naturally ends. Then we need some OS-specific
    > mechanism to link against boundaries. So windows that implements a
    > boundary trait gets the windows ABI, and trust naturally stops
    > there -- as with all boundary traits. I dont even understand the
    > question.
    > Owner (2026-07-16, CLOSES the question): the interpreter presumably
    > will get its own implementation of omega-specific boundary traits,
    > especially if we support WASM-like interpretation of Omega programs
    > (shipped up to an IR stage, rather than emitting binaries). In these
    > cases, we would, behind omega APIs, route to an interpreter version
    > of the impl. Now, as for user-defined boundaries, these would likely
    > just error out if there is no way to do anything with them. ie
    > trying to call some windows dll import or similar boundary in an
    > interpreter build -- theres simply no concept of this. This is
    > likely sufficient for our tests, as the idea of comparing program
    > output to interpreter output simply exists for our testing + future
    > WASM-like builds. There is 0 expectation that we run user programs,
    > built for specific targets, through the interpreter and have them
    > function.
    [CLOSED: today's behavior IS the design -- std boundaries get
    interpreter implementations (they have them); user-authored bindings
    error out interpreted and the differential skips them. No virtual-stub
    mechanism. The "interpreter as a WASM-like target" framing is recorded
    for the future IR-shipping story.]

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
    > Owner: I have no fucking clue why we keep doing this, can we not
    > declare this trait in omega::core or something? omega::std? Isnt
    > this a suuuper solved problem in programming?
    [DIRECTION TAKEN: yes -- declare it ONCE in omega::language::std and
    stop hand-spelling. std/console.omg exists but as a legacy `platform`
    block outside the boundary/effect system; the arc is promoting it to
    the canonical `boundary trait` with declared effect rows + per-target
    bindings, filesystem_host.omg-style. Filed in TASKS_FS; underway.]

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
    > Owner: The boot lattice is super fucking experimental and not
    > something to reference at all. Unless there is a consistency
    > problem WITHIN omega-rs itself, this is not a concern EVER. This
    > again feels like a super fucking solved problem, arent Console
    > operations fucking universal by now?
    [DIRECTION TAKEN: the lattice's input model carries no weight; there
    is no omega-rs-internal inconsistency -- the three samples simply
    call free functions that don't exist in real Omega. Under "Console
    ops are universal": std Console (the Q11 arc) gets the universal op
    set (read/write bytes + lines, error stream, exit_process) and the
    stdin samples are rewritten against it, which zeroes the
    samples_compile baseline.]
    > Owner (2026-07-16, EOF spelling): read_byte() returning -1 instead
    > of 0 on EOF sounds retarded, legacy non-ZII shit. even an
    > Option<i32> is better than -1 in my mind, magic numbers are fucking
    > retarded.
    [RESOLVED: shipped as a std sum, `data ByteRead { case Eof; case
    Byte(value: i32); }` in std/console.omg -- Eof is ordinal 0, so the
    ZII zero value IS end-of-input; no sentinel anywhere. (std Option<T>
    can't carry payloads yet, so the domain sum is the honest spelling
    today; fold into Option<i32> if/when variant payloads land there.)]

13. **Platform entries vs boundary traits — converge or give `platform`
    effect rows? (fs lane, 2026-07-17.)** Two declaration forms exist for
    host surfaces: `boundary trait X { machine f(..) effects row; }` (the
    general mechanism: effect rows, provides bindings, granted-build
    gating) and `platform Console { entry f(..); }` (std's console, wired
    through compiler host-op lowering, NO effect rows). Consequences
    today: (a) build.omg logging must hand-spell a BuildLog boundary trait
    because the granted-build gate needs DECLARED effects that platform
    entries cannot carry (the residue of your Q11 "declare it once"
    ruling); (b) the purity checker classifies `read_byte` as PURE ("no
    effects and no mutable out-parameters") -- wrong: it consumes a stdin
    byte. Probe-swept 2026-07-17: every current path REFUSES rather than
    elides, so this is not a live miscompile, but purity claims about
    effectful ops age badly. Options: (i) `platform` entries gain
    `effects` rows (small grammar addition; both forms live on); (ii)
    platform blocks CONVERGE onto boundary traits + std provides rows
    (one form, bigger migration -- the samples' `console: Console` field
    spelling can stay); (iii) status quo (hand-spelled BuildLog persists).
    This is the last open rung of the Q11/Q12 console arc.
