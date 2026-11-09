# Omega trap catalog — field notes

Measured findings from real programs, mostly against the **2026-08** compiler
(windows_x64 target). Each entry keeps its date and evidence. The compiler has
moved since — **re-verify a trap before designing around it**, and check whether
an upstream issue exists (e.g. CathedralOS/Omega#3 for the import-catalog case
bug). Repro artifacts lived in `omega-agentic/docs/compiler_bugs/` in a harness
checkout that no longer exists on this machine — the dated findings stand on
their own, but the artifacts aren't recoverable.

**Retired spelling in older entries.** Entries measured in 2026-08 write bounds
as a type suffix, `u32 [0..=N]`. The wiki has no such suffix (guide ch. 7): use
`requires` on parameters, `where` on data or a single case, `ensures` on
results, or a dominating guard. Where an entry's fix below still shows the old
suffix, translate it to the clause form and re-verify; the behaviour the entry
describes was measured with the old spelling.

General verification discipline: `omega --check` is necessary, not sufficient.
Several entries below pass `--check` cleanly and misbehave at runtime. For text
output, build and run: `./build/omega-program.exe | cat -A` — clean output is
text and `$` line-ends only. For values, print something that could only be
right if the logic genuinely ran (a zeroed buffer prints `0` whether or not the
fill happened — print `buf[5]` of a `buf[i] = i` fill and get `5`).

## Contents

- Text output and string literals
- Indexing and storage shapes
- Proof-obligation shapes
- Machine-call semantics
- Native calls and boundary traits
- Enum / tagged-union construction
- Dispatch on case values
- Harness and fixture traps
- Rewriting `if` — the most common port error
- Compile-error quick table

## Text output and string literals

### `Console::write_line(text: &[u8])` writes the entire array
There is no NUL-termination convention: a `[u8; 512]` buffer emits all 512
bytes, so any unused tail lands on the terminal verbatim.

- Constant text → pass the literal. Never poke ASCII integers byte by byte.
- Variable text → declare the buffer at the exact width, **pad with spaces
  (32)**, write it whole. Clear back to spaces between uses, not to zero.
- NUL-padding produces a screenful of control characters. Garbage output =
  this, usually.
- Runtime subslices to a boundary are not lowered — `write_line(self.buf[0..n])`
  fails "descriptor construction ... is not lowered for this shape yet", even
  with a guard proving the bound. Fixed-width space-padded buffers are the
  working idiom.

Canonical shape: `samples/cli/basics/multiplication_table/main.omg` —
space-fill the row, write digit bytes at computed offsets, `write_line(line)`.

### Literal assignment silently miscompiles without a Utf8 domain
Assigning a literal to a plain `[u8; N]` field **compiles clean and emits
garbage bytes at runtime** — no diagnostic. Declare the domain and mark the
field:

```omega
domain [u8; 16]::Utf8 requires valid_utf8(self);

data Main {
    plain: [u8; 16];            // self.plain = "plain           ";  -> prints ^@0|A<M-w>^?...
    typed: [u8; 16] in Utf8;    // self.typed = "typed           ";  -> prints "typed"
}
```

The literal must be padded to the field's exact width. Passing a literal
*directly* to a boundary (`write_line("text")`) needs no domain — only
assignment does.

### An escape sequence at the FIRST or LAST byte of a literal miscompiles
Even with `in Utf8` declared, a literal whose first or last character is an
escape (`\"` or `\\`) silently drops or corrupts that boundary byte at runtime
— `--check` passes, no diagnostic. Mid-literal escapes are fine (a dozen `\"`
in one literal proven). Found 2026-08-12: `"\"content\":\"OK\""` (escape at
position 0 AND last) dropped its final byte; `"z\"content\":\"OK\"z"` decoded
correctly. Workaround: never let a literal start or end with `\"`/`\\`; pad
both ends with a plain character. Verified with `Main::test_boundary_escape_literal`.

### A long run of identical bytes in a literal corrupts near the tail
A ~489-byte literal of 489 identical characters miscompiled its last 2 bytes at
runtime (correct domain, no escapes). A *diverse* 489-byte literal in the same
field decoded every byte correctly. Two other repeated-run literals (400×'A',
400×'B') did NOT show the bug — width and/or repeat-count matter, not pinned
further since real content never looks like this. Found 2026-08-12. If
generating a stress-test literal, use varied content, not `"X" * N`.

### Printing a computed number
No printf, no working itoa. A runtime value reaches the console one digit at a
time via `narrow_*` into a fixed buffer:

```omega
use omega::language::core::numeric_conversion;

data Main { console: Console; count: u32 in Wrapping; out: [u8; 8]; }

self.out[0] = 99;                                          // 'c'
self.out[1] = 61;                                          // '='
self.out[2] = narrow_u32_to_u8_trapping(self.count + 48);  // the digit
self.out[3] = 32;                                          // pad with SPACE
self.console.write_line(self.out);
```

`+ 48` turns 0-9 into the ASCII digit; multi-digit via `n / 10`, `n % 10`.
`narrow_u32_to_u8_trapping` for `u32`, `narrow_i32_to_u8_trapping` for `i32`;
`as u8`/`as u32` casts also exist.

**Not lowered:** writing through a `&mut [u8; N]` parameter alias
(`machine f(out: &mut [u8; 32]) { out[i] = ...; }`) fails codegen with "needs
runtime storage write lowering", though `--check` passes while the machine is
unreachable. Write through `self` instead.

## Indexing and storage shapes

### Nested field in an index SUM is a hard error
`self.grp.buf[i]` and `self.grp.buf[i + 1]` are fine. But
`self.grp.buf[self.grp.cur + i]` — a *nested* field participating in the sum —
fails: "index `self.grp.cur + i` is a computed expression, not yet supported as
an indexed operand (it would silently read 0 or no-op); compute it into a field
first". The equivalent top-level shape `self.buf[self.cursor + i]` compiles.
Workaround per the message: hoist the sum into its own field, index by that
field alone — costs one extra state per use site. Found 2026-08-19; it's why
fields used in index sums (a write cursor) are worth leaving top-level when
grouping other fields into concern structs.

### Struct-array indices have a narrower inline-arithmetic budget
`self.byte_array[95 + self.a]` compiles; `self.message_array[self.save_start + 1]`
does not — the same computed-expression error at only 2 terms. Same fix: assign
the sum to a field, index by the field.

### Computed indices beyond ~2 terms are rejected
`95 + self.a + self.b + j` fails as an indexed operand. A 2-term sum (one field
+ one loop var) is the proven shape; anything wider gets hoisted through
single-assignment fields (`self.base = 95 + self.a;` then `self.base2 =
self.base + self.b;`, index with `self.base2 + j`). Give hoisted offset fields
a *tight* declared bound — the prover uses the declared bound, not the narrower
value actually held, so a loose bound makes downstream sums unprovable.

### Read-and-write of dynamically-indexed nested fields needs a scratch field
`self.arr[i] = self.other.nested_arr[idx].field[i]` (both directions in one
statement) → "state mutation ... needs runtime storage write lowering". Fix:
whole-array-copy the source into a plain scratch field first
(`self.scratch = self.other.nested_arr[idx].field;`), then index `self.scratch`.
Caught at compile time — unlike the DllImport buffer-reuse gap below.

## Proof-obligation shapes

Exact arithmetic is a proof obligation (spec: `numeric_values.md`), not a lint.
These are cases where the prover needs an explicit extra hop — none are
compiler bugs, all measured 2026-08-11.

- **(Landed since, 2026-09-27/28.) A guard's narrowed bound now carries into
  the state it lands in** for free machines passing the value as a state
  parameter: `transition k < 28u64 { true -> wipe(line, k) ... }` lets `wipe`
  index `line[k]` and compute `k + 1u64` with no extra proof. The original
  2026-08 finding was about `self.x` guards as a machine's first un-nested
  transition, where targets couldn't prove values derived from the narrowed
  field; if that shape still fails, give the guard its own named state and
  re-derive the same guard inside each target.
- **`u8 as u32` doesn't carry an implied `[0..=255]` bound.** The cast result
  is an opaque, effectively-unbounded u32 to the prover. `(self.raw[i] as u32) *
  256` fails "exact arithmetic ... may overflow". Fix: assign the cast into a
  `u32 [0..=255]` field first, then do the arithmetic.
- **An explicit full-width bound can be REJECTED where unbounded is accepted.**
  `field: u32 [0..=4294967295]` failed to prove an `as u32` cast into it;
  `field: u32` with no bound compiled. If a cast won't prove into a wide
  bounded field, drop the bound annotation.
- **A compound `||` transition guard can fail as non-exhaustive.**
  `transition (x == 13) || (x == 10) || (x == 34) || (x == 92) { true -> ...
  false -> ... }` reported "can fall through: no arm matches when every guard
  is false" despite true/false being exhaustive. A chain of states each testing
  ONE equality and falling through on `false` compiles immediately — prefer
  that shape from the start.
- **Casting a domain-tagged int needs an explicit strip + named conversion.**
  `self.len_wide = self.recv_len0 as u32` where `recv_len0: i32 in Wrapping`
  fails "implicit domain weakening ... drops 'Wrapping'". Fix: `as i32` first
  to strip the domain, then `narrow_i32_to_u32_trapping`/`_wrapping` from
  `numeric_conversion`. Call the trapping variant only after a local `<= 0`
  guard — it traps on genuine negatives (e.g. a real `recv()` error).
- **A running write-cursor updated at N points needs N single-assignment
  fields.** A field's bound is checked against EVERY assignment site, so
  reusing `seg_base` across three running-total updates fails — the RHS's
  provable max exceeds the bound even though the live value is small. Split
  into `seg1_base`/`seg2_base`/`seg3_base`, each assigned exactly once;
  downstream index sums then combine independently-tight bounds.
- **Bounded-loop state params need explicit ranges, and the narrowing bound
  must be a literal.** `state s(&mut self, i: u32 [0..=N])` — small-N loops can
  omit it, large-N cannot ("may overflow" on `i + 1`). The body state reached
  only when `i < N` held needs `[0..=N-1]` — declaring both `[0..=N]` fails on
  the `i + 1` re-entry. And guarding on a *field* (`i < self.some_len`) doesn't
  narrow `i` the way a literal (`i < 256`) does, even with a bounded field —
  the prover doesn't compose "i < field" with "field <= 256". Fix: outer state
  guards the literal max, inner state compares against the real field using the
  already-narrowed `i`.

## Machine-call semantics

### Self-receiver machines drop extra arguments and return values at runtime
**LANDED — re-verify before designing around it.** Measured 2026-08-11 on the
old compiler; as of the 2026-09 pull, `tests/omega/pass/calls/` contains
`runtime_value_call_return_types_exit`, `runtime_dispatch_binary_call_argument_exit`,
and `runtime_enum_self_method_exit`, covering exactly this surface. Free
machines were unaffected then and remain the simplest shape — prefer them
anyway for code that doesn't need a receiver. Original note:

Measured 2026-08-11. A machine with a `self`/`&self`/`&mut self` receiver **plus
any other explicit parameter** reads that parameter back as its type's zero
default at runtime — whether passed via ordinary call or a `transition { _ ->
state(v) }` hop. Return values are dropped the same way. `--check` accepts all
of it cleanly. (Repro was `compiler_bugs/self_receiver_args_dropped/` in the
retired harness.)

A **free machine** (no receiver) with the same shape is unaffected —
`machine second_of(a: u32, b: u32) -> u32 { b }` returns 5 correctly.

**The only reliable workaround:** self-receiver machines hand off data through
`self` fields, never parameters or return values — one zero-extra-arg machine
writes `self.f`; the caller reads `self.f` directly. Plain field-to-field
assignment within a state body is unaffected — only crossing a machine-call
boundary with a non-`self` value loses it. Before trusting such a machine,
verify it carries data (print a value that only comes out right if the argument
made it), not just that it doesn't crash.

**Does NOT apply to `Binding::DllImport` host calls** — a machine satisfying a
boundary trait via DllImport genuinely receives args and returns (verified with
real WinSock calls). The bug is specific to Omega-to-Omega calls.

## Native calls and boundary traits

### `via Binding::DllImport(dll, symbol)` works — with sharp edges
Real native DLL calls verified end-to-end 2026-08-11 (live TCP connection via
`ws2_32.dll`, real HTTP response). Not a documented stdlib feature — a real
compiler capability:

```omega
boundary trait Winsock {
    machine ws_socket(af: i32, type_: i32, protocol: i32) -> i64;
}
machine ws_socket_native(af: i32, type_: i32, protocol: i32) -> i64
    satisfies Winsock::ws_socket via Binding::DllImport("ws2_32.dll", "socket");

data Main { ws: Winsock; }
// self.ws.ws_socket(2, 1, 6)  -- AF_INET, SOCK_STREAM, IPPROTO_TCP
```

Found empirically (guide chapter 19 covers `via` bindings but not these):

- **A bare `&mut [u8]`/`&[u8]` parameter is rejected** — "cannot use safe
  slice ... as a default-native parameter". Wrap bytes in a nominal `data`
  struct (`data NetBuf { data: [u8; 4096]; }`), then `&mut NetBuf`/`&NetBuf`.
  `self.buf.data[i]` reads/writes normally.
- **Host-call args must be "simple values"** — a local, field, parameter, or
  literal; not an inline computed expression. Assign to a field/local first.
- **The declared return type must be non-void**, even for void C functions —
  a void `satisfies` fails "no encodable call sequence". Declare `-> i32` and
  ignore it.
- **Do not loop-copy elements out of a buffer a DllImport call just filled into
  another nested buffer.** `self.accumulator.data[i] = self.scratch.data[i]`
  silently left `accumulator` all-zero while a counter in the same loop was
  correct — the loop ran, the copy didn't take. Context-sensitive; two
  narrowing attempts didn't isolate it. Workaround: every native fill gets its
  own destination buffer (never reuse one scratch across calls) and move data
  with whole-array assignment (`self.a = self.b`), not indexed copy loops. N
  chunks = N buffers + N whole-array copies.
- **Fixed native structs work directly**: `sockaddr_in`-style scalar fields
  (`family: u16; port_be: u16; addr_be: u32; zero: u64;`) with `&TheStruct` —
  only byte *arrays* need wrapping. Byte-swap network-order values by hand into
  little-endian storage (port 11545 net-order → stored LE value 6445;
  127.0.0.1 → 16777343).

### One provider slot per (trait, target) — by design
Exactly ONE `satisfies` impl may exist per build, selected via
`build.omg`'s `b.select_provider<Trait, Provider>()`. Two providers fail
`--check`: "slot `Greeter` has two covering provider plans ... choose one in
build.omg". Designed-for failure (canaries/fail/providers/), not a bug. Don't
reach for `boundary trait` to unify a canned path and a live path in one build
— keep separate concrete types and let callers pick by name. `boundary trait`
is right only when exactly one impl is ever selected per build (platform
primitives: `Console`, `Winsock`, `FileIO`).

### Catalog-symbol library case matters — the ">5 args" misdiagnosis
A `DllImport` whose SYMBOL is in the compiler's Windows import catalog
(`WINDOWS_IMPORT_ROWS` in `omega-rust/omega/representations/calling-conventions/
src/hosts/windows.rs`: `Kernel32.dll`, `User32.dll`, `Gdi32.dll`, `msvcrt.dll`)
must use
the catalog's EXACT library-string case. `"kernel32.dll"` (lowercase) passes
`--check` but fails the `--target` build with "compiler planned-import
instruction #N does not retain its exact call/data/storage relocation set"
(`validate_compiler_planned_import_relocations`). Catalog case builds and runs;
`"KERNEL32.dll"` fails again. Non-catalog symbols (`GetFileAttributesA`) are
case-agnostic.

Root-caused 2026-08-15 after two false theories: a 7-param `CreateFileA`
failure was recorded as a ">5 args ceiling" (a 5-arg non-catalog control
passed), and a drifting instruction index across compiler pulls was misread as
scale-sensitivity. The `_l*` legacy file API detour worked only because `_l*`
names are absent from the catalog. Rules: (1) relocation-set error → check
catalog membership before suspecting arg counts or scale; (2) use exact catalog
case — `CreateFileA`/`ReadFile`/`WriteFile` are usable again; (3) upstream
issue CathedralOS/Omega#3 filed — lowercase may work someday; until then the
rule stands.

Real-API gotcha, not a compiler bug: **`_lopen` does not support `OF_CREATE`**
(confirmed against MSDN + a .NET P/Invoke control outside Omega). `_lcreat`
creates/truncates; `_lopen` only opens existing files.

## Enum / tagged-union construction

### Non-first case with payload fields is broken — no workaround found
**LANDED — re-verify before designing around it.** Measured 2026-08-11 on the
old compiler; the 2026-09 pull moved case-field store/transfer into
`tests/omega/pass` (e.g. `calls/by_value_case_param_self_write_exit`). The
tag+payload escape hatch below still costs nothing and stays exhaustive, but
it is no longer forced. Original note:

Measured 2026-08-11. Constructing any case OTHER than a `data`'s first case
into an array element (`self.arr[i] = MyEnum::Second { ... }`) reads back as
the FIRST case with all-zero fields — construction silently didn't take. In
isolation the same shape instead fails to build (`UnloweredCaseLiteralField ...
unclassified zero-width row`). Which manifestation is context-sensitive;
not chaseable from outside the compiler. Wrapping fields in a `[copy]` struct
didn't fix it — not a field-count problem, specific to non-first cases. A plain
`[copy]` struct with 2+ fields is unaffected.

**Confirmed safe (narrows the bug):** a non-first case with NO fields (unit
case `case AwaitingTools;`) in a plain self field, matched via `transition
self.field { MyEnum::Case -> ... }`, round-trips fine (index 2 of 7 tested).
Whether payload or array-element storage is the trigger wasn't isolated —
treat ANY case with fields as unsafe, any unit case as safe.

**Working escape hatch:** unit-only enum for the tag + flattened `[copy]`
struct for the payload:

```omega
data Role [copy] { case User; case Assistant; case ToolResult; }
data Message [copy] where content_len <= 256 { role: Role; content: [u8; 256]; content_len: u32; has_tool_call: bool; }
```

Same information; array-element construction and transition-matching both use
independently-proven paths. Costs wasted space and loses per-case payload
exhaustiveness — a fine trade for message-log-style unions. Proven in
production (omega-agentic `Message`/`Role`), not just isolation.

## Dispatch on case values

### `match` is value-only by design — case shapes go through `transition`
Measured 2026-09-27: `match phase { Phase::Red -> ... }` fails `--check` with
"match structural, range, and binding patterns are not supported by value
dispatch yet". The wiki (guide ch. 6) confirms the split: value-position
`match` arms admit value and wildcard patterns only; record, domain and
guarded spellings belong to `transition` dispatch. So this isn't a pending
feature to wait for. Use subject-form transition dispatch, which binds
payloads and must cover every case:

```omega
transition reply {
    Reply::Hit { value } -> keep(value)
    Reply::Miss          -> miss()
}
```

or, for payload-free cases (their equality is intrinsic), a guard list:

```omega
machine next(phase: Phase) -> Phase {
    transition {
        phase == Phase::Red    -> Phase::Green
        phase == Phase::Green  -> Phase::Yellow
        _                      -> Phase::Red
    }
}
```

Case equality (`phase == Phase::Red`) works in guards; `data Phase [copy] {
case Red; ... }` construction, storage, and parameter passing all check clean.

## Harness and fixture traps

### A killed `omega --check` orphans the process — later checks "hang" on the snapshot lock
Measured 2026-09-27. Terminating a shell that ran `omega.exe --check` leaves
the compiler running; it keeps the lock under
`%LOCALAPPDATA%/CathedralOS/Omega/source/v1/external-local-sources/local-snapshots/`.
Every subsequent check against the same dependency silently waits — no output
for minutes, indistinguishable from a compiler hang. Fix:
`tasklist //FI "IMAGENAME eq omega.exe"`, `taskkill //PID <pid> //F`, retry.

Each fresh package location also re-snapshots and re-checks its whole
dependency closure — a first `--check` on a fixture that `depend`s on std runs
~5 minutes; identical-location reruns are seconds. Reuse one fixture dir in a
harness instead of `mkdtemp` per case.

## Rewriting `if` — the most common port error

`the 'if' statement is retired; dispatch is transition <guard> { true -> ... _ -> ... }`:

```omega
// Rust idiom -- rejected
if self.n > 2 { big(); } else { small(); }

// Omega -- a guard picks a state, each state ends in a transition
// (verified with --check 2026-09-28, Omega main 86353504ea; build.omg binds `main`)
use omega::language::core::process_exit;
use omega_language_std::process_exit;
use omega_language_std::console;

machine main() reaches Console + ProcessExit {
    transition { _ -> check(3u32) }

    state check(n: u32) {
        transition n > 2u32 { true -> big() _ -> small() }
    }
    state big() {
        Console::write_line("big");
        transition { _ -> done() }
    }
    state small() {
        Console::write_line("small");
        transition { _ -> done() }
    }
    state done() {
        ProcessExit::exit_process(0);
    }
}
```

### A receiver entry with a `Binding<Console>` field can't also call `ProcessExit` statically
Measured 2026-09-28. `data Main { console: Binding<Console>; ... }` with
`machine Main::main(&mut self) reaches Console + ProcessExit` calling
`self.console.write_line(...)` and then `ProcessExit::exit_process(0)` fails:
"selected ProgramEntry establishment rejoins 0 Terminal attachment identities;
expected one". The repo's own fixture notes that the transitional native entry
envelope admits one provider-backed service carrier per attachment root.
Either use a free entry with static calls for both services (above), or give
the receiver both bindings (`console: Binding<Console>; exit:
Binding<ProcessExit>;`) as `tests/omega/pass/host/process_exit_i32_status_*`
does (interpreter-scoped there).

## Compile-error quick table

| Error | Cause | Fix |
|---|---|---|
| `expected ':', found '='` | `let v = expr;` | `let v: u32 = expr;` — type required |
| `expected ':', found '+'` on return | `transition { _ -> u32 { a + b } }` | `transition { _ -> (a + b) }` — parenthesize |
| `exact arithmetic ... may overflow` | unbounded operands | bound them (`requires x in 0..=1000`, a data/case `where`, or a dominating guard), widen with `as`, or `u32 in Wrapping` |
| `the 'if' statement is retired` | Rust idiom | transition dispatch above |
| `descriptor construction ... is not lowered for this shape yet` | runtime subslice to a boundary | fixed-width space-padded buffer |
| `index ... is a computed expression, not yet supported` | field/struct in index sum, or >2 terms | hoist the sum into its own field |
| `needs runtime storage write lowering` | write through `&mut` alias; nested dynamic-index copy | write through `self`; scratch-field copy |
| `no encodable call sequence` | void-returning `satisfies` machine | declare non-void return, ignore it |
| `cannot use safe slice ... as a default-native parameter` | bare `&[u8]`/`&mut [u8]` in DllImport | wrap in `data NetBuf { data: [u8; N]; }` |
| `implicit domain weakening ... drops 'Wrapping'` | `as`-cast off a domain-tagged int | `as` to plain type first, then `narrow_*` |
| `does not retain its exact call/data/storage relocation set` | catalog symbol, wrong library case | `"Kernel32.dll"`/`"msvcrt.dll"` exact case |
| `cannot prove transition argument 'i + 1' satisfies bounded parameter` | loop-state bounds too loose (2026-08, old suffix spelling) | let the loop state's guard (`i < N`) supply the body's bound; state any remaining bound with `requires` on the state; re-verify |
| `can fall through: no arm matches when every guard is false` | compound `\|\|` guard | chain single-comparison states |
| `two covering provider plans` | two `satisfies` for one trait | `select_provider` one; or separate concrete types |
