# Tasks — Time (`std::time`)

> **START HERE (scoped 2026-07-06; ALL judgment calls settled with Zach same day —
> see Design decisions D11/D12/D14/D15/D16 and Resolved questions).** Nothing is
> built yet. This file is the source of truth: it carries the settled design, the
> per-target binding plan, and the phased task list. The feature does not start
> from zero — the compiler already has a `Clock` host capability (`sleep`,
> `tick_count`) bound on windows_x64 with passing canaries; see **Current state**.
> Zach roped TWO compiler pre-rungs into this workstream: the **i128 literal
> carrier** (D14) and **const-v0** (D15) — they are rungs 1–2 below and unblock
> the surface as designed (`Duration::MAX` at true `u64::MAX`, type-scoped
> consts). The first pure-Omega rung is #3.
>
> **WORKING RULES.** Consult `wiki/language_guide/*` before assuming a language
> feature; prefer ZII / arena / `Handle` / `HandleSpan`; check Rust source for
> `std::time` parity (we copy the general API shape, not the mechanisms); full
> human-word names (`from_seconds`, `checked_subtract`, `subsecond_nanoseconds`)
> — NO abbreviations (`secs`/`nanos`/`sub`) in the Omega surface; C symbols only
> in the per-target binding tables. windows_x64 is this workstream's TESTED
> target; keep darwin/aarch64 and linux structurally ready. Build canaries that
> RUN and assert behavior. Gates that must stay green: Console lowering;
> `omega-instruction-selection`/`omega-relocations`/`omega-calling-conventions`
> crate tests; `canary_suite` interpreter coverage; the existing host canaries
> (`runtime_tick_count_monotonic_exit`, `runtime_tick_paced_marquee_exit`).
> Verify canary_suite regressions by an **A/B failure-set diff** (the ~85
> standing failures are pre-existing from other workstreams), never by raw count.
>
> **PUSH TO MAIN.** After committing: `git fetch origin`; if behind, `git rebase
> origin/main` + re-verify; then `git push origin HEAD:main`. This workstream
> SHARES hot files with the fs and render workstreams — see **Coordination**
> before touching the calling-convention tables, and note rung 1 (literal
> carrier) sweeps proof/validation/folding files other threads touch: land it as
> ONE focused commit, rebase small and often.

## North star

A **serious, ergonomic `std::time`** with **parity to Rust's `std::time`**,
differing where Omega is better:

- `Result<T, E>` → bespoke `data` case enums, error as the first (ZII-zero) case.
- Rust's panicking operator arithmetic (`+`/`-` on `Duration`) → does not exist.
  The checked and saturating forms ARE the API; Omega's exact-arithmetic
  obligation makes unproven overflow a compile error anyway.
- Rust's ambient clock (`Instant::now()` from anywhere) → clock reads live on an
  explicit `Time` capability data (holding the `TimeHost` boundary trait), the
  same way `Filesystem` holds `FilesystemHost`. `Duration`, `Instant`, and
  `SystemTime` are pure `[copy, zero_init]` values with no authority.
- Rust's `u128` returns (`as_nanos`) → checked/saturating `u64` variants
  (no `u128` TYPE in the language; `u64` nanoseconds cap at ~584 years, fine for
  everything except whole-`Duration` nanosecond totals, which get result cases).

Two layers, exactly the fs pattern: portable wrapper in
`omega/language/std/time.omg` over a raw `TimeHost` boundary trait in
`omega/language/std/time_host.omg`, lowered per-target via binding rows +
`insert_platform_lowering`.

## Current state (corrected 2026-07-06 after direct verification)

- **Compiler:** `HostCapability::Clock` exists
  (`omega-calling-conventions/src/lib.rs`) with operations `Sleep` and
  `TickCount`, bound in `windows.rs` to `Kernel32 Sleep` / `GetTickCount64`.
  The "value-returning ops are x86_64-only" fence is actually a **Gui-only
  x86_64 gate inside `windows.rs`** — Clock rows are bound unconditionally, and
  the fs workstream already proved value-returning host calls on darwin/aarch64.
  No fence lift is needed for new value-returning Clock ops; darwin is table
  work.
- **Canaries:** `canaries/pass/host/runtime_tick_count_monotonic_exit` and
  `runtime_tick_paced_marquee_exit` pin sleep + tick_count natively. Apps
  declare `boundary trait Clock { machine sleep(milliseconds: u32); machine
  tick_count() -> u64; }` inline; lowering matches on MACHINE NAME (the `"*"`
  type wildcard), so trait names are cosmetic and new operation names must be
  collision-free across all host-lowered names.
- **Interpreter:** Clock is a **virtual clock** (`evaluator.rs` ~2270): `sleep`
  advances `virtual_ticks` by the milliseconds (no real delay), `tick_count`
  increments by 1 per read. The interpreter filesystem is likewise a virtual
  in-memory FS. D12 builds on this, not on real host clocks.
- **std:** no time module. `std/console.omg` carries `sleep(milliseconds: u32)`
  and it is **LIVE, not a stub** — 11 animation samples + the
  `runtime_sleep_exit` canary call `self.console.sleep(...)` (it lowers because
  host lowering matches the machine name). D16 kills it. Filesystem `Metadata`
  exposes `modified_secs`/`accessed_secs`/`created_secs`/`changed_secs` as `i64`
  Unix epoch seconds — `SystemTime` must be constructible from those.
- **Language features verified for this design:** scalar data-field range
  refinements are parse + WRITE-ENFORCED + read-narrowed (fail canaries
  `range/guarded_copy_bound_too_wide` et al. pin over-wide scalar-field writes)
  — D3 is sound as written. Case-enum payloads may be multi-field structs
  (fs `MetadataResult` precedent, construction + transition-arm destructuring).
  Struct-payload native delivery fixed 2026-07-06 (windows-verified). No
  `Ordering` type exists anywhere in `omega/language` — time mints the first.
- **Known gaps this workstream now OWNS (Zach 2026-07-06):** integer literals
  are carried as `i64` from parse, so `u64` magnitudes above `i64::MAX` are
  unspellable (D14); `const` declarations are design-settled
  (`static_root_and_constants.md`, 2026-07-04) but unbuilt (D15).
- **No wall clock anywhere; no sub-millisecond monotonic source anywhere.**
  `GetTickCount64` is millisecond-unit, ~10–16ms granularity.

## Design decisions (ratified with Zach 2026-07-06)

- **D1. Full human-word API.** `from_seconds` / `from_milliseconds` /
  `from_microseconds` / `from_nanoseconds`, `checked_add` / `checked_subtract`,
  `saturating_add` / `saturating_subtract`, `subsecond_nanoseconds`,
  `duration_since`, `saturating_duration_since`. Rust's `secs`/`nanos`/
  `checked_sub` spellings are banned.
- **D2. Two layers.** Portable `time.omg` wrapper over a raw `time_host.omg`
  `boundary trait TimeHost` seam (mirrors D2/D-fs-host-module from TASKS_FS.md).
- **D3. Duration representation.**
  `seconds: u64; subsecond_nanoseconds: u32 [0..=999_999_999];` — the field
  range refinement carries the normalization invariant (write-enforcement
  VERIFIED live, see Current state), and ZII zero IS `Duration::ZERO`. No
  `new()` machine needed; literal construction is range-checked.
- **D4. No panicking arithmetic, no operator forms.** `checked_*` machines
  return a result data (`case Overflow;` first); `saturating_*` return plain
  values. Internally these are guard transitions — the house "checked" idiom.
- **D5. No u128 TYPE.** `as_nanoseconds`/`as_microseconds`/`as_milliseconds`
  totals come in `checked_` and `saturating_` forms returning `u64`. Internal
  conversion math stays in guarded `u64`; the normalization identity
  `(ticks % frequency) * 1_000_000_000 / frequency` fits `u64` for every real
  clock frequency (proof bound: frequency ≤ 18_446_744_073 Hz, guarded once at
  calibration). With D14, `u64::MAX` LITERALS become spellable — the type
  system still has no u128.
- **D6. Instant normalizes at `now()`.** The host returns raw ticks + a
  ticks-per-second calibration; the wrapper converts ONCE into normalized
  (seconds, subsecond_nanoseconds). Duration math stays trivial; per-platform
  weirdness is confined to one machine. `Instant` is opaque-by-convention: its
  fields mean "since an unspecified monotonic epoch" and only differences are
  meaningful.
- **D7. SystemTime ZII = Unix epoch.** `seconds_since_unix_epoch: i64` +
  `subsecond_nanoseconds: u32 [0..=999_999_999]`. `UNIX_EPOCH` is the ZII zero,
  spelled as a type-scoped const (D15). `duration_since` returns a case enum
  whose error case carries the backwards amount. `from_unix_seconds(i64)`
  bridges filesystem `Metadata`.
- **D8. Extend the existing `Clock` capability**, do not mint a second one.
  `sleep` and `tick_count` keep their exact current bindings and canaries; new
  operations are added alongside.
- **D9. Clock reads are capability-explicit.** `now()`, `system_time_now()`,
  `elapsed_since()`, and `sleep_for()` live on `data Time { host: TimeHost; }`.
  Rust's `Instant::elapsed(&self)` (ambient clock read) intentionally has no
  equivalent; the Cathedral no-ambient-authority stance applies.
- **D10. No float surface in v1.** `from_seconds_f64` etc. deferred. (The
  aarch64 scalar float ABI landed both directions in render fire 6, so this is
  now a SCOPE choice, not an ABI blocker; the type-blind f32/f64 folder issues
  also argue for waiting.)
- **D11. Wall clock = ONE raw `u64` read + per-target constants (SETTLED,
  replaces the buffer op).** The lowering layer cannot express post-call
  arithmetic (`PlatformCallData` = None | FirstTextArgument |
  MutableOutputBuffer; `append_newline` is pre-call text shaping), so ALL
  epoch/unit conversion lives in the wrapper, fed by per-target constants:
  `wall_clock_raw() -> u64` (a single non-tearing read),
  `wall_clock_units_per_second() -> u64`, `wall_clock_epoch_offset_seconds()
  -> u64`. `unix_seconds = raw / units - epoch_offset`;
  `subsecond_nanoseconds = (raw % units) * 1_000_000_000 / units` (same D5
  proof bound). One read per `system_time_now()` — no second-boundary tearing,
  no buffer, and the wall-clock math becomes frequency-shaped exactly like D6.
- **D12. Interpreter = VIRTUAL clock (SETTLED, replaces "real host clock").**
  The interpreter's Clock is already virtual and its fs is a virtual in-memory
  FS; new ops extend that model, they do not fight it. `monotonic_ticks`
  derives from `virtual_ticks` (so `sleep` advances the monotonic clock —
  elapsed-vs-sleep canaries hold); wall clock = a fixed seed constant +
  the virtual advance. CONSEQUENCE: interpreter time canaries assert EXACT
  values (stronger than inequalities); native canaries assert inequalities
  (monotonic non-decreasing; elapsed ≥ sleep duration). This also dissolves
  the render-workstream conflict: their headless interpreter stub covers
  Gui/Input only — TIME owns interpreter Clock semantics.
- **D13. Ordering.** `data Ordering { case Less; case Equal; case Greater; }`
  minted in `time.omg` (verified: no Ordering exists anywhere; promote to a
  shared std module when a second consumer appears), plus
  `is_less_than`/`is_greater_than` bool conveniences. Struct equality via the
  synthesized `Equatable` `equals`. Do not depend on `==` for payload-bearing
  case enums (payload-aware equality is still pending language-side).
- **D14. ANONYMOUS integer literals (ROPED IN; corrected 2026-07-06 — Zach: an
  i128 carrier "misses the point", e.g. the day a u128 type exists).** A
  literal is an UNINTERPRETED PAYLOAD — normalized digits + radix + sign, the
  squalr model — and is NEVER a numeric value until a USE gives it a type.
  There is no numeric carrier to outgrow; a future u128/u256 changes nothing
  at the literal layer.
  - Representation: `Integer(i64)` in the three tree enums (syntax / typed /
    symbol-resolved `expression.rs`) → the payload. The ONLY way to get a
    number out is `value_for(target_type) -> value | diagnostic` (fit-check at
    the use; the error names the literal and the target type). No bare getter.
  - Consumers WITH type context — binding boundaries
    (`integer_literal_constraints` → `[v,v]` interval ∩ target range, which
    already exists), typed operands, codegen immediates (both ISAs already
    take full 64-bit bit patterns) — deanonymize; mechanical.
  - Consumers WITHOUT type context DEFER. In particular the type-blind const
    folder must NOT fold anonymous literals; folding moves behind typing
    ("the type rides on the constant"). This MERGES the open const-fold
    sign-miscompile class ([[shift-right-signedness-const-fold]] /
    [[decision-17-const-fold-domain-hole]]) into this rung — it is no longer
    orthogonal. Canary the defer tail: a previously-folded constant now
    reaching lowering UNFOLDED must either still lower or reject loudly,
    never miscompile.
  - Negative literals: the sign joins the payload (parse-time `wrapping_neg`
    dies with the eager i64).
  - `IntegerRange { minimum, maximum }` (omega-proof `obligations.rs:378`)
    widens as an INTERNAL prover detail (precision for the widest SUPPORTED
    type's range) — the prover's width is contained in one struct and is no
    longer the language's literal ceiling.
  - Pin at implementation: the default-type rule for an annotation-less
    `let x = 5` (today's rule, unchanged — the binding IS the use); what an
    interval means in a genuinely untyped context (should not exist post-audit).
  - Scope: the same ~284 accesses / ~167 logical sites / 123 files, but each
    is an AUDIT (type context? → deanonymize; none? → defer), not a rename.
    Budget MULTI-FIRE; the defer-caused fold changes are the regression risk,
    A/B the full canary tail every fire.
- **D15. const-v0 (ROPED IN; design settled 2026-07-04, unclaimed in TASKS.md).**
  Implement the `const` declaration per `static_root_and_constants.md`, scoped
  to LITERAL-ONLY initializers: scalars and struct-literals-of-literals, free
  or `Type::`-scoped, pure-value check, never a data member. Enough for
  `Duration::ZERO/MAX`, `SystemTime::UNIX_EPOCH`, and Cathedral's EFI constants.
  The full build-time-evaluation arc (effect-free machines in const position)
  stays where it is in TASKS.md. Fallback if this rabbit-holes (revert +
  document): v1 ships ZII-as-ZERO/UNIX_EPOCH + a `maximum()` value machine.
- **D16. `Console::sleep` is KILLED (Zach: kill outright if possible).** Remove
  the entry from `std/console.omg`; migrate the 11 animation samples + the
  `runtime_sleep_exit` canary to a clock capability field (distinct type beside
  Console — same-type contained-machine aliasing does not apply). Lowering
  matches machine name, so behavior/exit codes are unchanged by the re-plumb.
  Fall back to deprecation (entry stays, marked deprecated, no new users) ONLY
  if a migration face resists; document why.

## The surface (end state)

### `omega/language/std/time.omg`

```omega
data Duration [copy, zero_init] {
    seconds: u64;
    subsecond_nanoseconds: u32 [0..=999_999_999];
}

const Duration::ZERO: Duration = Duration { seconds: 0, subsecond_nanoseconds: 0 };
const Duration::MAX: Duration = Duration { seconds: 18446744073709551615, subsecond_nanoseconds: 999999999 };
// (MAX's seconds literal needs D14; ZERO/UNIX_EPOCH need only D15.)

data DurationResult [copy, zero_init] {
    case Overflow;
    case Ok(duration: Duration);
}

data NanosecondsResult [copy, zero_init] {
    case Overflow;
    case Ok(nanoseconds: u64);
}

data Instant [copy, zero_init] {
    seconds: u64;
    subsecond_nanoseconds: u32 [0..=999_999_999];
}

data InstantResult [copy, zero_init] {
    case Overflow;
    case Ok(instant: Instant);
}

data SystemTime [copy, zero_init] {
    seconds_since_unix_epoch: i64;
    subsecond_nanoseconds: u32 [0..=999_999_999];
}

const SystemTime::UNIX_EPOCH: SystemTime = SystemTime { seconds_since_unix_epoch: 0, subsecond_nanoseconds: 0 };

data SystemTimeDifference [copy, zero_init] {
    case Backwards(amount: Duration);
    case Ok(duration: Duration);
}

data Ordering [copy, zero_init] {
    case Less;
    case Equal;
    case Greater;
}

data Time {
    host: TimeHost;
    // calibration captured on first use; fields carry the proof bounds:
    // monotonic_frequency: u64 [1..=18_446_744_073];
    // wall_units: u64 [1..=18_446_744_073]; wall_epoch_offset_seconds: u64;
}
```

**Duration machines** (pure value, `&self` unless noted): `from_seconds(u64)`,
`from_milliseconds(u64)`, `from_microseconds(u64)`, `from_nanoseconds(u64)`;
`as_seconds() -> u64` (truncating), `subsecond_nanoseconds() -> u32`,
`subsecond_milliseconds() -> u32`, `subsecond_microseconds() -> u32`;
`checked_as_milliseconds` / `checked_as_microseconds` / `checked_as_nanoseconds
-> NanosecondsResult` and `saturating_` twins returning `u64`; `is_zero() ->
bool`; `checked_add` / `checked_subtract` `(other: Duration) ->
DurationResult`; `saturating_add` / `saturating_subtract` `(other: Duration) ->
Duration`; `checked_multiply(factor: u32) -> DurationResult`,
`saturating_multiply(factor: u32) -> Duration`, `divide(divisor: u32) ->
DurationResult` (first case also covers divisor 0 — guard, never a runtime
bounds error); `compare(other: &Duration) -> Ordering`, `is_less_than`,
`is_greater_than`; `equals` via `Equatable`.

**Instant machines** (pure value): `duration_since(earlier: Instant) ->
Duration` (saturating at `ZERO`, total — the ZII-friendly default),
`checked_duration_since(earlier: Instant) -> DurationResult`,
`checked_add(duration: Duration) -> InstantResult` /
`checked_subtract(duration: Duration) -> InstantResult`.

**SystemTime machines** (pure value): `from_unix_seconds(seconds: i64) ->
SystemTime`; `duration_since(earlier: SystemTime) -> SystemTimeDifference`;
`checked_add` / `checked_subtract` with `Duration`.

**Time machines** (capability, `&mut self`): `now() -> Instant`,
`system_time_now() -> SystemTime`, `elapsed_since(start: Instant) -> Duration`,
`sleep_for(duration: Duration)` (chunks through the millisecond host op,
saturating each chunk at `u32` max).

### `omega/language/std/time_host.omg`

```omega
// Raw per-OS time boundary. Value-returning scalars ONLY (D11) — portable
// SEMANTICS are fixed here and each target's binding/lowering must meet them.
boundary trait TimeHost {
    // Monotonic, never-decreasing tick counter since an unspecified epoch.
    machine monotonic_ticks() -> u64;
    // Ticks per second for monotonic_ticks. Stable for the process lifetime.
    machine monotonic_ticks_per_second() -> u64;
    // Wall clock: ONE read, raw platform units since the platform epoch.
    machine wall_clock_raw() -> u64;
    // Units per second for wall_clock_raw (constant per target).
    machine wall_clock_units_per_second() -> u64;
    // Platform-epoch → Unix-epoch shift, in seconds (constant per target).
    machine wall_clock_epoch_offset_seconds() -> u64;
    // Existing op, unchanged: millisecond sleep (already bound on windows).
    machine sleep(milliseconds: u32);
}
```

## Reference — per-target bindings

| Contract op | windows_x64 (TESTED) | darwin/aarch64 (ready) | linux |
|---|---|---|---|
| `monotonic_ticks` | `QueryPerformanceCounter` (out-param `LARGE_INTEGER` → item 1) | `clock_gettime_nsec_np(CLOCK_UPTIME_RAW)` (direct `u64`, clockid constant arg → item 2) | deferred — `clock_gettime` writes a timespec (needs arithmetic; no shape) |
| `monotonic_ticks_per_second` | `QueryPerformanceFrequency` (out-param → item 1) | constant `1_000_000_000` (item 2) | deferred |
| `wall_clock_raw` | `GetSystemTimePreciseAsFileTime` (out-param `FILETIME` = one LE `u64` → item 1) | `clock_gettime_nsec_np(CLOCK_REALTIME)` (direct `u64`) | ABSENT — clean `UnsupportedHostCall` until an arithmetic-capable shape exists |
| `wall_clock_units_per_second` | constant `10_000_000` (item 2) | constant `1_000_000_000` | — |
| `wall_clock_epoch_offset_seconds` | constant `11_644_473_600` (1601→1970) | constant `0` | — |
| `sleep` | `Kernel32 Sleep` — **already bound** | **DONE (render fire 23):** `poll(NULL, 0, ms)` | `nanosleep` syscall |

**Conversion math (ALL in the wrapper, per D11):**

- Monotonic: `seconds = ticks / frequency`; `subsecond_nanoseconds =
  (ticks % frequency) * 1_000_000_000 / frequency` — the multiply fits `u64`
  because `ticks % frequency < frequency ≤ 18_446_744_073` (guard once when
  calibration is captured; QPF is typically 10 MHz, POSIX is 10^9).
- Wall: `unix_seconds = raw / units - epoch_offset`; `subsecond_nanoseconds =
  (raw % units) * 1_000_000_000 / units` (same bound on `units`).
- Instant subtraction borrow: if `later.subsecond_nanoseconds <
  earlier.subsecond_nanoseconds`, borrow one second and add `1_000_000_000`.

**Compiler engineering items:**

1. **Out-param scalar result shape (windows).** QPC, QPF, and FILETIME deliver
   through a pointer argument. New shape: reserve a stack slot, pass its
   address, load the `u64` after the call, store via the normal result path.
   Follow the 3-site lockstep discipline (`widths.rs` + relocation offsets +
   encoder) that `dereferences_result()` (fs D9) and the fire-6 float return
   both used — the precedent is cheap and well-trodden. x86_64 first (tested
   target).
2. **Constant-result ops + constant-argument injection.** A lowering that
   materializes a per-target constant `u64` as the op result with NO call
   (windows wall units/epoch, darwin/linux frequencies) — new
   `PlatformCallData` variant, trivially encodable; and constant ARG injection
   for darwin's `_nsec_np` clockid values. Precedent for lowering-layer data
   shaping: `FirstTextArgument { append_newline }`.
3. **Enum + table additions:** new `HostOperation` variants with
   `from_name`/`name` arms in `omega-calling-conventions/src/lib.rs`
   (names collision-free across ALL host-lowered names — lowering matches on
   machine name); import rows + `insert_platform_lowering` in `windows.rs`
   (then `darwin.rs`); check `omega-relocations` host-operation instruction
   records and any driver trust-root count assertions.
4. **NO aarch64 fence lift needed** (corrected): the x86_64 gate in
   `windows.rs` wraps Gui ops only; Clock rows bind unconditionally and fs
   proved aarch64 value returns. Darwin Clock additions are table work.
5. **D14 anonymous literals** — scope in D14; multi-fire, each fire green;
   canaries: `let x: u64 = 18446744073709551615` exact round-trip natively +
   interp; a FAIL canary for a magnitude no type fits AND for a u64-range
   literal bound to i64 (fit-check at use names literal + target); a fold-defer
   canary (an expression the folder used to fold sign-blindly now either
   lowers correctly or rejects loudly).
6. **D15 const-v0** — declaration parse (free + `Type::`-scoped), symbol
   resolution, literal-only initializer evaluation, pure-value check,
   use-site materialization (a const use folds to its literal value / struct
   literal). Canaries: scalar const in arithmetic + guard; struct const
   (`Duration::ZERO`-shaped) constructed and field-read; FAIL canary for a
   non-literal initializer.

## Resolved questions (were O1–O3)

- **O1 (glue math home): RESOLVED → wrapper + constants (D11).** Verified: the
  lowering layer cannot express post-call arithmetic; option (c) — raw platform
  units + per-target constants — is now the contract itself.
- **O2 (calibration): RESOLVED → store in `Time` fields.** Scalar field range
  write-enforcement is verified live, so range-refined calibration fields carry
  the D5 proof bound; capture via the fs entry-capture pattern (host result
  into a field in the machine ENTRY, then guard the stored field).
- **O3 (Ordering home): RESOLVED → mint in `time.omg`** (D13); nothing else
  defines or needs one today.

## Next steps

1. [~] **Anonymous integer literals (D14) — FIRE B LANDED 2026-07-06.**
   `omega_core::literals::IntegerLiteral` (canonical spelling payload, Arc-str,
   `value_i64()` as the sole window, NO unbounded getter) rides all three tree
   enums; parser validates + canonicalizes, any magnitude parses; negative fold
   is textual (mirrors Float). ~180 consumer sites audited: typed positions
   read through the window, context-less consumers DEFER (folder leaves
   oversize unfolded; equality folds compare by VALUE so `5 == 0x5` still
   folds true; borrow-overlap treats oversize as may-alias — conservative,
   never unsound). The validation gate (omega-validation/src/literals.rs, a
   whole-expression-table scan) makes any surviving oversize literal ONE clear
   "exceeds the i64 range" error. PROVEN: i64::MIN now directly spellable
   (`runtime_i64_min_literal_exit` runs natively, exit 70, was a parse error);
   u64::MAX parses and gates (`u64_literal_above_i64_max` rescoped). VERIFIED:
   canary_suite 627/1 (the 1 = pre-existing build_machine_wrong_arity
   missing-files), samples_compile = the 4 documented knowns only, all crate
   gates green.
   **FIRE C LANDED 2026-07-07: u64-magnitude literals ACCEPTED at u64-classed
   direct stores.** `value_u64()` + `bits_u64()` (the 8-byte truth) on
   `IntegerLiteral`; the gate became position-aware (u64_blessed_literals —
   direct assignment RHS into `u64`/`usize`/`addr` places, resolved through
   `declared_place_type_raw` + unwrapped primitive; everything else still one
   clear error naming the accepted alternative). The write-path constant
   resolvers (writes/static_values.rs) and the interpreter literal arm
   materialize `bits_u64()` — sound ONLY because the gate is precise (a
   `u32 in Wrapping` slot bypasses the interval store-check; the gate is what
   stands between it and silent truncation — grow acceptance and its consumer
   in the SAME change, per the module header). PROVEN:
   `runtime_u64_max_literal_exit` — `self.mask = 18446744073709551615` then
   `+ 1 == 0` wrapping round-trip, native exit 70, interp via
   ACTIVE_PASS_CANARIES; fail canary rescoped to the i64-target face.
   REMAINING (fire D+): more accepted positions as std needs them (typed
   `let`, struct-literal fields — `Duration::MAX`'s const initializer will
   need the CONST path once D15 lands), `IntegerRange` i128 widening for
   u64-range PROOF facts (today an oversize literal carries no facts — fine
   for Wrapping, rejects Exact arithmetic), then fold-behind-typing (the
   const-fold sign class merge).
2. [~] **const-v0 (D15) — TYPE-SCOPED CONSTS LANDED 2026-07-07.**
   `const Type::NAME: T = <literal>;` parses (contextual keyword, item
   position; `pub` prefix rides the existing wrapper). Consts exist ONLY until
   symbol resolution: validated at their item arm
   (syntax-trees-to-symbol-resolved/src/constant.rs — literal-only
   initializers, duplicate check, case-constructor collision check,
   free-floating rejected with "scope it to a type"), then every `Type::NAME`
   path substitutes a FRESH initializer copy at expression lowering. Typed
   trees, validation, proofs, backends, interp never see a const — the
   copied-at-each-use semantics of the brief, and zero downstream churn.
   PROVEN: `constants/runtime_scoped_const_exit` (scalar const through
   exact-arithmetic interval proofs + struct const constructed into a field,
   native exit 70, interp via ACTIVE_PASS_CANARIES); 3 fail canaries pin the
   v0 boundaries. REMAINING: free-floating consts (needs the local-shadowing
   walk); declaration-site type conformance for UNUSED consts (uses are
   checked post-substitution today); richer initializers = the build-time
   evaluation arc (TASKS.md). `Duration::ZERO`/`MAX`/`UNIX_EPOCH` are now
   spellable — MAX's u64::MAX field additionally needs the struct-literal
   field position blessed for u64-magnitude literals (D14 fire D).
3. [~] **Duration pure-value core — NATIVE + INTERPRETER VERIFIED 2026-07-07
   (differential canary `time/runtime_duration_core_exit`, exit 70 both
   engines).** `omega/language/std/time.omg` ships `Ordering`, `Duration`,
   `DurationResult`, `Duration::ZERO` (const-v0), `as_seconds`, `is_zero`,
   `checked_add` (wrapped-compare overflow idiom — no u64::MAX literal
   needed), `checked_subtract` (borrow path), `saturating_subtract` (ZERO
   clamp), `compare`/`is_less_than`/`is_greater_than`. VALUE-MACHINE AUTHORING
   RULES (documented in time.omg + canary; the day's misdiagnoses corrected):
   receivers route through the FIRST field of their type (the KNOWN same-type
   receiver-aliasing bug has a value-call flavor — repro
   canaries/pending/time/value_machine_receiver_field_postentry, deep fix now
   high-leverage per TASKS.md); payload field values stay cascade-safe (bare
   `param % literal`; a `(cast) % literal` payload value is SILENTLY DROPPED
   by the parallel write cascade — Exact-casts belong in entry lets);
   entry-only field reads + params-only post-entry states; operand-tagged
   Wrapping lets; inline `%`-bounded ranged stores. backend_report renders
   convert widths in BYTES (`as i8->i8` = 8-byte identity).
   FIRE E (2026-07-07): `Duration::MAX` (true u64::MAX seconds — D14 fire D
   blessed u64-classed STRUCT-LITERAL FIELDS and bits-enabled the
   leaf/mutation static-integer readers for terminal payload writes) +
   `saturating_add` (clamps at MAX) + the construction FAIL canary
   (`time/duration_subsecond_range_rejected`), all differential-verified.
   ALSO FIXED: the interpreter compared u64 operands SIGNED (the
   wrapped-compare idiom broke at u64::MAX, interp only) — comparisons now
   take an UNSIGNED witness from declared types (Frame-recorded u64-classed
   locals/params, cast targets, self-fields; positive-witness-only, so signed
   compares cannot regress). REMAINING interp face (filed in TASKS.md):
   unsigned div/mod/shift-right and min/max on msb-set u64. REMAINING rung-3
   surface: `from_*` unit constructors (receiverless type-scoped value calls
   — verify resolution); `checked_as_*` totals; `checked_multiply` /
   `divide`.
4. [ ] **`TimeHost` seam + interpreter support** (`time_host.omg`, D12):
   virtual-clock interpreter implementations for the five value ops;
   interpreter canaries asserting EXACT values (monotonicity across a virtual
   sleep; `system_time_now()` == seed + advance).
5. [ ] **Windows bindings** — engineering items 1–3: out-param shape,
   constant-result shape, QPC/QPF/`GetSystemTimePreciseAsFileTime` rows +
   lowerings. Native canaries mirroring `runtime_tick_count_monotonic_exit`:
   `runtime_instant_elapsed_exit` (t1 = now, sleep 30ms, elapsed_since(t1) ≥
   30ms → distinct exit codes per failure arm) and
   `runtime_system_time_after_2026_exit`
   (`system_time_now() > from_unix_seconds(1_767_225_600)`).
6. [ ] **Wrapper completion** — `Instant`/`SystemTime`/`Time` machines over the
   seam, entry-capture pattern, calibration guard (O2).
7. [x] **Kill `Console::sleep` (D16) — DONE 2026-07-06, killed OUTRIGHT (no
   deprecation needed).** Entry removed from `std/console.omg`; the 11 samples
   + `runtime_sleep_exit` migrated onto inline `boundary trait Clock` + a
   `clock` field (plain `Clock` name coexists fine with the std console import
   — the old `Clock2` spelling was unnecessary caution). VERIFIED:
   samples_compile A/B-clean (only the 4 documented pre-existing Windows-host
   failures; all 11 migrated samples run with correct exits + output
   assertions); canary_suite 626 pass / 1 fail, the 1 being the pre-existing
   `build_machine_wrong_arity` missing-files red (never committed to git —
   fs-thread issue, task chip spawned), `runtime_sleep_exit` green.
8. [ ] **Sample** — `samples/cli/systems/elapsed_timer`: measure a real
   workload with `Instant`, print milliseconds, exit code proves the
   elapsed-≥-sleep chain (the stopwatch sample stays simulated-tick).
9. [ ] **Filesystem interop** — `SystemTime::from_unix_seconds` consumed from
   `Metadata` timestamps; later, fs-side `modified() -> SystemTime` parity
   (coordinate with the fs workstream; their file, their call).
10. [ ] **darwin/aarch64** — `_nsec_np` rows (constant-arg injection) +
    constant-result rows; `usleep` stays render's (do not duplicate). Native
    confirmation needs a Mac.
11. [ ] **linux** — monotonic/wall rows stay DEFERRED (timespec out-param needs
    arithmetic no lowering shape provides); document the gap honestly rather
    than a broken row. `nanosleep` row only.

## Coordination

- **Render workstream (TASKS_RENDER.md):** owns darwin `Clock.sleep` binding
  (their item 7). SETTLED HERE (D12): the interpreter headless stub (their
  item 9) covers **Gui/Input only** — time owns interpreter Clock semantics
  (virtual clock). Reflect that in TASKS_RENDER.md item 9 when next edited.
- **Filesystem workstream (TASKS_FS.md):** shares the calling-convention hot
  files (`lib.rs`, `windows.rs`, `darwin.rs`, `linux.rs`), `canary_suite.rs`
  (`ACTIVE_PASS_CANARIES`), and established the wrapper patterns this plan
  reuses (entry-capture, D9 lockstep rule). Rebase small and often.
- **TASKS.md claims:** the u64-literal gap (fence catalog) and the const task
  are now CLAIMED by this workstream — annotated there 2026-07-06.
- **Cathedral:** keep `TimeHost` narrow and capability-shaped — the
  freestanding target has no host layer, and this contract is what Cathedral
  would later implement via UEFI `GetTime` / TSC calibration. Nothing here may
  grow an ambient-authority shortcut (D9). const-v0 (D15) directly serves
  Cathedral's EFI constants.
