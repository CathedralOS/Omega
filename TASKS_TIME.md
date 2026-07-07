# Tasks — Time (`std::time`)

> **START HERE (fresh workstream, scoped 2026-07-06).** Nothing is built yet.
> This file is the source of truth: it carries the settled design, the per-target
> binding plan, and the phased task list. The feature does not start from zero —
> the compiler already has a `Clock` host capability (`sleep`, `tick_count`)
> bound on windows_x64 with passing canaries; see **Current state**. Phase 1
> (the pure-value `Duration` core) needs no host or compiler work at all and can
> start immediately.
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
> before touching the calling-convention tables.

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
  (no `u128` in the language; `u64` nanoseconds cap at ~584 years, fine for
  everything except whole-`Duration` nanosecond totals, which get result cases).

Two layers, exactly the fs pattern: portable wrapper in
`omega/language/std/time.omg` over a raw `TimeHost` boundary trait in
`omega/language/std/time_host.omg`, lowered per-target via binding rows +
`insert_platform_lowering`.

## Current state

- **Compiler:** `HostCapability::Clock` exists
  (`omega-calling-conventions/src/lib.rs`) with operations `Sleep` and
  `TickCount`, bound in `windows.rs` to `Kernel32 Sleep` / `GetTickCount64`.
  Value-returning Clock/Gui/Input ops are fenced to x86_64 (clean
  `UnsupportedHostCall` elsewhere). The fs workstream has since proven
  value-returning host calls on darwin/aarch64 for Filesystem ops, so the fence
  is precedent to follow, not a wall.
- **Canaries:** `canaries/pass/host/runtime_tick_count_monotonic_exit` (the
  repo's first value-returning host import) and
  `runtime_tick_paced_marquee_exit` pin sleep + tick_count natively. Apps
  declare `boundary trait Clock { machine sleep(milliseconds: u32); machine
  tick_count() -> u64; }` inline; lowering matches on MACHINE NAME (the `"*"`
  type wildcard), so trait names are cosmetic and new operation names must be
  collision-free across all host-lowered names.
- **std:** no time module. `std/console.omg` carries a signature-only
  `sleep(milliseconds: u32)` entry. Filesystem `Metadata` exposes
  `modified_secs`/`accessed_secs`/`created_secs`/`changed_secs` as `i64` Unix
  epoch seconds — `SystemTime` must be constructible from those.
- **No wall clock anywhere; no sub-millisecond monotonic source anywhere.**
  `GetTickCount64` is millisecond-unit, ~10–16ms granularity.

## Design decisions (ratified in scoping, user reviews later)

- **D1. Full human-word API.** `from_seconds` / `from_milliseconds` /
  `from_microseconds` / `from_nanoseconds`, `checked_add` / `checked_subtract`,
  `saturating_add` / `saturating_subtract`, `subsecond_nanoseconds`,
  `duration_since`, `saturating_duration_since`. Rust's `secs`/`nanos`/
  `checked_sub` spellings are banned.
- **D2. Two layers.** Portable `time.omg` wrapper over a raw `time_host.omg`
  `boundary trait TimeHost` seam (mirrors D2/D-fs-host-module from TASKS_FS.md).
- **D3. Duration representation.**
  `seconds: u64; subsecond_nanoseconds: u32 [0..=999_999_999];` — the field
  range refinement carries the normalization invariant the compiler enforces at
  every construction site, and ZII zero IS `Duration::ZERO`. No `new()`
  machine needed; literal construction is already range-checked.
- **D4. No panicking arithmetic, no operator forms.** `checked_*` machines
  return a result data (`case Overflow;` first); `saturating_*` return plain
  values. Internally these are guard transitions — the house "checked" idiom.
- **D5. No u128 workarounds.** `as_nanoseconds`/`as_microseconds`/
  `as_milliseconds` totals come in `checked_` and `saturating_` forms returning
  `u64`. Internal conversion math stays in guarded `u64`; the normalization
  identity `(ticks % frequency) * 1_000_000_000 / frequency` fits `u64` for
  every real clock frequency (proof bound: frequency ≤ 18_446_744_073 Hz,
  guarded once at calibration).
- **D6. Instant normalizes at `now()`.** The host returns raw ticks + a
  ticks-per-second calibration; the wrapper converts ONCE into normalized
  (seconds, subsecond_nanoseconds). Duration math stays trivial; per-platform
  weirdness is confined to one machine. `Instant` is opaque-by-convention: its
  fields mean "since an unspecified monotonic epoch" and only differences are
  meaningful.
- **D7. SystemTime ZII = Unix epoch.** `seconds_since_unix_epoch: i64` +
  `subsecond_nanoseconds: u32 [0..=999_999_999]`. `UNIX_EPOCH` is the ZII zero,
  spelled as a type-scoped const. `duration_since` returns a case enum whose
  error case carries the backwards amount (Rust's `SystemTimeError` shape).
  `from_unix_seconds(i64)` bridges filesystem `Metadata`.
- **D8. Extend the existing `Clock` capability**, do not mint a second one.
  `sleep` and `tick_count` keep their exact current bindings and canaries; new
  operations are added alongside.
- **D9. Clock reads are capability-explicit.** `now()`, `system_time_now()`,
  `elapsed_since()`, and `sleep_for()` live on `data Time { host: TimeHost; }`.
  Rust's `Instant::elapsed(&self)` (ambient clock read) intentionally has no
  equivalent; the Cathedral no-ambient-authority stance applies.
- **D10. No float surface in v1.** `from_seconds_f64` etc. wait for the float
  ABI (render workstream). Integer constructors cover real use.
- **D11. Wall-clock reads must not tear.** Seconds and subsecond nanoseconds
  derive from ONE underlying clock read per `system_time_now()` (a two-op
  split reading the clock twice can straddle a second boundary).
- **D12. Interpreter = real host clock.** New ops get interpreter
  implementations over the interpreter's own runtime clock (reference
  semantics, same as fs running real syscalls). Host-clock canaries assert
  inequalities (monotonic non-decreasing; elapsed ≥ sleep duration); pure-value
  Duration canaries assert exact values.
- **D13. Ordering.** `data Ordering { case Less; case Equal; case Greater; }`
  shared vocabulary type, plus `is_less_than`/`is_greater_than` bool
  conveniences. Struct equality via the synthesized `Equatable` `equals`
  (normalized fields make field-equality correct). Do not depend on `==` for
  payload-bearing case enums (payload-aware equality is still pending
  language-side).

## The surface (end state)

### `omega/language/std/time.omg`

```omega
data Duration [copy, zero_init] {
    seconds: u64;
    subsecond_nanoseconds: u32 [0..=999_999_999];
}

const Duration::ZERO: Duration = Duration { seconds: 0, subsecond_nanoseconds: 0 };
const Duration::MAX: Duration = Duration { seconds: 18446744073709551615, subsecond_nanoseconds: 999999999 };

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

data SystemTime [copy, zero_init] {
    seconds_since_unix_epoch: i64;
    subsecond_nanoseconds: u32 [0..=999_999_999];
}

const SystemTime::UNIX_EPOCH: SystemTime = SystemTime { seconds_since_unix_epoch: 0, subsecond_nanoseconds: 0 };

data SystemTimeDifference [copy, zero_init] {
    case Backwards(amount: Duration);
    case Ok(duration: Duration);
}

data Time {
    host: TimeHost;
    // calibration captured on first use (ticks_per_second, guarded ≤ 18_446_744_073)
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
`checked_subtract(duration: Duration) -> InstantResult` (same two-case shape).

**SystemTime machines** (pure value): `from_unix_seconds(seconds: i64) ->
SystemTime`; `duration_since(earlier: SystemTime) -> SystemTimeDifference`;
`checked_add` / `checked_subtract` with `Duration`.

**Time machines** (capability, `&mut self`): `now() -> Instant`,
`system_time_now() -> SystemTime`, `elapsed_since(start: Instant) -> Duration`,
`sleep_for(duration: Duration)` (chunks through the millisecond host op,
saturating each chunk at `u32` max).

### `omega/language/std/time_host.omg`

```omega
// Raw per-OS time boundary. Value-returning scalars only; portable SEMANTICS
// are fixed here and each target's binding/lowering must meet them.
boundary trait TimeHost {
    // Monotonic, never-decreasing tick counter since an unspecified epoch.
    machine monotonic_ticks() -> u64;
    // Ticks per second for monotonic_ticks. Stable for the process lifetime.
    machine monotonic_ticks_per_second() -> u64;
    // Wall clock as a single non-tearing read: Unix-epoch seconds (i64) and
    // subsecond nanoseconds, delivered per the buffer layout in the module.
    machine read_wall_clock(buffer: &mut [u8]) -> i32;
    // Existing op, unchanged: millisecond sleep (already bound on windows).
    machine sleep(milliseconds: u32);
}
```

(`read_wall_clock` buffer layout: bytes 0..8 little-endian `i64` Unix seconds,
bytes 8..12 `u32` subsecond nanoseconds; returns 0 on success, negative on
failure. If open question O1 resolves toward lowering-layer transforms, the
wrapper decode stays identical on every target.)

## Reference — per-target bindings

| Contract op | windows_x64 (TESTED) | darwin/aarch64 (ready) | linux (ready) |
|---|---|---|---|
| `monotonic_ticks` | `QueryPerformanceCounter` (out-param `LARGE_INTEGER`) | `clock_gettime_nsec_np(CLOCK_UPTIME_RAW)` (direct `u64` return, needs clockid constant arg) | `clock_gettime(CLOCK_MONOTONIC, &timespec)` syscall (out-param) |
| `monotonic_ticks_per_second` | `QueryPerformanceFrequency` (out-param) | constant `1_000_000_000` | constant `1_000_000_000` |
| `read_wall_clock` | `GetSystemTimePreciseAsFileTime` (out-param `FILETIME`) + epoch/unit conversion | `clock_gettime_nsec_np(CLOCK_REALTIME)` or `clock_gettime` | `clock_gettime(CLOCK_REALTIME, &timespec)` |
| `sleep` | `Kernel32 Sleep` — **already bound** | `usleep(ms * 1000)` — **render workstream item 7** | `nanosleep` syscall |

**Conversion math (lives per D6 in the wrapper, or per O1 in lowering):**

- Tick normalization: `seconds = ticks / frequency`; `subsecond_nanoseconds =
  (ticks % frequency) * 1_000_000_000 / frequency`. The multiply fits `u64`
  because `ticks % frequency < frequency ≤ 18_446_744_073` (guard once when
  calibration is captured; QPF is typically 10 MHz, POSIX is 10^9).
- FILETIME (u64 count of 100ns units since 1601-01-01 UTC):
  `unix_seconds = filetime / 10_000_000 - 11_644_473_600`;
  `subsecond_nanoseconds = (filetime % 10_000_000) * 100`.
- Instant subtraction borrow: if `later.subsecond_nanoseconds <
  earlier.subsecond_nanoseconds`, borrow one second and add `1_000_000_000`.

**Compiler engineering items (the real machinery of this feature):**

1. **Out-param scalar result shape.** QPC, QPF, FILETIME, and timespec all
   deliver their value through a pointer argument, not the return register. A
   `PlatformCallData` variant (stack slot + post-call load, or the fs
   stat-buffer pattern where the Omega side passes `&mut [u8]`) is needed; it
   is reused by every row in the table above except darwin's `_nsec_np` calls
   and the already-bound ops. Precedents: `MutableOutputBuffer` (read_line),
   fs `stat` buffer decode, D9 in TASKS_FS.md (`dereferences_result()`
   post-call load, including its three-site lockstep width rule).
2. **Constant-argument injection** (darwin clockid values for `_nsec_np`) and
   **constant result** (fixed 10^9 frequency on darwin/linux). Precedent for
   lowering-layer data shaping: `FirstTextArgument { append_newline }`.
3. **Enum + table additions:** new `HostOperation` variants with
   `from_name`/`name` arms in `omega-calling-conventions/src/lib.rs`; import
   rows + `insert_platform_lowering` calls in `windows.rs` (then `darwin.rs`,
   `linux.rs`); check `omega-relocations` host-operation instruction records
   and any driver trust-root count assertions.
4. **aarch64 value-returning Clock ops** — lift the x86_64 fence following the
   path fs proved for Filesystem value returns (coordinate with render).

## Open questions

- **O1. Where does per-OS glue math live?** D6 puts tick normalization in the
  portable wrapper (frequency makes it target-agnostic). The FILETIME
  epoch/unit conversion is not frequency-shaped, so it must live either (a) in
  a lowering-layer transform (recommended; `append_newline` precedent), or
  (b) in a per-target instruction sequence behind the `read_wall_clock`
  contract, or (c) by weakening the contract to raw platform units + a
  per-target constant pair (epoch offset, units per second) the wrapper
  consumes. Implementer resolves against the codebase; the contract semantics
  in `time_host.omg` must not change whichever way this goes.
- **O2. `Time` calibration capture.** Guarding `ticks_per_second ≤
  18_446_744_073` once and storing it in `Time` fields vs. re-reading per
  `now()`. Storing is preferred (one guard, prover-visible field range);
  confirm the field-range spelling the prover accepts.
- **O3. `Ordering` home.** `std/time.omg` for now, or promote to a shared std
  module immediately if the fs workstream wants it too.

## Next steps

1. [ ] **Duration pure-value core** (`time.omg`, no host, no compiler work):
   data + consts + constructors + accessors + checked/saturating arithmetic +
   compare. Interpreter canaries under `canaries/pass/time/` asserting exact
   values: construction, `checked_add` overflow arm taken, saturating clamps at
   `MAX`/`ZERO`, unit conversions, borrow-carrying subtraction, `divide` by
   zero takes the first case. Native runs on windows_x64 (pure value code —
   should be table-free).
2. [ ] **`TimeHost` seam + interpreter support** (`time_host.omg`, D12):
   interpreter implementations for the three new ops; interpreter canary for
   `Time::now()` monotonicity and `system_time_now() > from_unix_seconds(1_767_225_600)`
   (2026-01-01).
3. [ ] **Windows bindings** — engineering items 1–3: out-param shape,
   QPC/QPF/`GetSystemTimePreciseAsFileTime` rows + lowerings, O1 resolution.
   Native canaries mirroring `runtime_tick_count_monotonic_exit`:
   `runtime_instant_elapsed_exit` (t1 = now, sleep 30ms, elapsed_since(t1) ≥
   30ms → distinct exit codes per failure arm) and
   `runtime_system_time_after_2026_exit`.
4. [ ] **Wrapper completion** — `Instant`/`SystemTime`/`Time` machines over the
   seam, using the fs entry-capture pattern (host result into a field in the
   machine ENTRY, then guard on the stored field).
5. [ ] **Sample** — `samples/cli/systems/elapsed_timer`: measure a real
   workload with `Instant`, print milliseconds, exit code proves the
   elapsed-≥-sleep chain (the stopwatch sample stays simulated-tick; a new
   sample avoids rewriting it).
6. [ ] **Filesystem interop** — `SystemTime::from_unix_seconds` consumed from
   `Metadata` timestamps; later, fs-side `modified() -> SystemTime` parity
   (coordinate with the fs workstream; their file, their call).
7. [ ] **darwin/aarch64** — engineering item 4 (fence lift) + `_nsec_np` rows
   (needs O2 constant-arg injection) + `usleep` (render workstream owns
   `Clock.sleep` darwin — do not duplicate). Native confirmation needs a Mac.
8. [ ] **linux** — binding table rows only (structural readiness, same policy
   as fs next-step #5).
9. [ ] **`Console::sleep` reconciliation** — the signature-only stub in
   `std/console.omg` either delegates to `Time::sleep_for` or is removed in
   favor of it (user call).

## Coordination

- **Render workstream (TASKS_RENDER.md):** owns darwin `Clock.sleep` binding
  (item 7) and the interpreter headless `Gui`/`Input`/`Clock` stub (item 9).
  Agree before adding ANY darwin Clock rows or interpreter Clock behavior —
  D12 (real interpreter clock) must not fight their headless stub; the stub
  can virtualize `sleep` while `monotonic_ticks` stays real.
- **Filesystem workstream (TASKS_FS.md):** shares the calling-convention hot
  files (`lib.rs`, `windows.rs`, `darwin.rs`, `linux.rs`), `canary_suite.rs`
  (`ACTIVE_PASS_CANARIES`), and established the wrapper patterns this plan
  reuses (entry-capture, buffer decode, D9 lockstep rule). Rebase small and
  often; enum arms and table rows are append-mostly but not conflict-free.
- **Cathedral:** keep `TimeHost` narrow and capability-shaped — the
  freestanding target has no host layer, and this contract is what Cathedral
  would later implement via UEFI `GetTime` / TSC calibration. Nothing here may
  grow an ambient-authority shortcut (D9).
