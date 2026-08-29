# The Alpha calling convention

> The foundation of Beta-the-language. This is the one thing that turns "an
> assembler" into "a language you can write a compiler in." The historically
> Beta-named Alpha assembler does not use it: every value that must survive a
> `call` lives at a *fixed global
> address* (see the `524288+` block in `assembler.alpha`), so there are no locals,
> no parameters, and **no recursion**. A real procedure needs a per-call *frame*.
> This document is that frame discipline.

## Two stacks

The Alpha VM already has a stack — but it is **hidden**. `call` pushes a return
address and `ret` pops it, on a stack the program cannot address. That is all the
VM gives us for control flow, and it is enough: the program never touches return
addresses.

For *data* (parameters, locals, saved registers) the program maintains its **own**
explicit stack in memory:

```
   control  ──►  the VM's hidden return-address stack   (call / ret)
   data     ──►  an explicit data stack in memory        (program-managed)
```

Recursion works because each call's frame is a fresh region on the data stack, and
its return address is a fresh slot on the hidden stack. Neither clobbers the other.

## Current canonical-compiler memory profile

The Alpha-written Beta compiler currently emits this physical layout:

```text
[0, 262140)                 Alpha tape payload
[262144, 1048576)           downward generated data stack (guard still open)
[1048576, 2097152)          reserved separation
[2097152, 35651584)         biased 32 MiB source-visible raw memory
[35651584, 67108864)        hidden-return-stack allowance (limit still open)
```

Every emitted byte/word access now checks the logical 32 MiB bound, rejects a
signed-negative Word address, and adds the raw base before touching Alpha
memory. Thus logical address zero is initially zero rather than tape byte zero.
This is not yet the complete non-alias theorem: generated `r15` reservations
must still be guarded at `262144`, and semantic call depth must be bounded so
the hidden Alpha return stack cannot descend below `35651584`.

## Register roles

| Register | Role |
| --- | --- |
| `r15` | **stack pointer (sp)** — an offset into memory, grows **downward**. Initialized once at startup. Doubles as the expression-evaluation stack. |
| `r14` | **frame pointer (fp)** — fixed for a procedure's lifetime; params/locals live at `[fp - 8 - 8*slot]`, so they stay addressable while `sp` moves for expression temporaries. The compiler uses it; hand-written code may skip it (the examples below do, tracking offsets by hand). |
| `r0`–`r3` | the first four **arguments** (further args are pushed on the data stack — deferred). |
| `r0` | the **return value**. |
| `r0`–`r5` | **caller-saved** scratch. A caller that needs one of these to survive a `call` saves it in its own frame first. |
| `r6`–`r12` | **callee-saved**. A callee that uses one must save it on entry and restore it before `ret`. |
| `r13` | **word-size constant**. Generated-program startup sets it to `8`; generated procedures preserve it and shared frame/stack macros consume it directly. |

(This supersedes gamma's current "vars `a`–`j` live in fixed registers `r6`–`r15`"
scheme: under a real convention, locals live in **frames**, not fixed registers,
and `r15` is reserved as the dsp.)

## Frame protocol

The compiler brackets each procedure with an **fp-based** prologue and epilogue
(schematic; immediates are loaded through a scratch register — see any generated
`build/*.asm`):

```
proc:
        ; prologue
        sub   r15, r13              ; \  push the caller's fp (`r13 == 8`)
        store r15, r14              ; /
        mov   r14, r15              ; fp = sp   (the frame base)
        sub   r15, framesize        ; allocate params + locals below fp
        ; store args r0..r(n-1) into param slots [fp - 8 - 8*k]
        ; ... body: params/locals at [fp - 8 - 8*slot]; sp moves freely for temps ...
        ; epilogue
        mov   r15, r14              ; sp = fp   (discard locals + temporaries)
        load  r14, r15              ; \  pop the caller's fp
        add   r15, r13              ; /
        ret
```

The data stack is balanced per call (the frame is fully reclaimed by `sp = fp`),
and control returns ride the VM's separate hidden stack — so recursion just works.

**Leaf procedures** that hold no state across a call skip the frame entirely —
compute in `r0`–`r5` and `ret`. **Spilling across a call**: a caller-saved value
(e.g. an argument in `r0`) that must outlive a `call` is stored into the frame
before the call and reloaded after.

## Proven on the seed

Three hand-written examples exercise the convention end-to-end (assembled by
the Alpha assembler, then run on the Alpha seed):

- [`factorial.alpha`](../alpha/assembler/examples/factorial.alpha) — single recursion;
  `factorial(5)` exits **120**. The frame holds one slot: `n`, saved across the
  recursive call.
- [`fib.alpha`](../alpha/assembler/examples/fib.alpha) — **tree** recursion (two recursive
  calls per frame); `fib(10)` exits **55**. The frame holds two slots and never
  relies on a register surviving a call.
- [`gcd.alpha`](../alpha/assembler/examples/gcd.alpha) — **two parameters** (`r0`, `r1`) and
  a **tail call that needs no frame**; `gcd(48, 36)` exits **12**. Shows the
  leaf/tail case where a procedure skips the frame entirely.

Build one through `source/alpha/assembler/build.sh`, for example with
`source/alpha/assembler/examples/factorial.alpha`.

## Remaining limits

- **>4 arguments** — spill the rest onto the data stack (left-to-right; caller
  cleans up).
- **Language control shape** — Beta now has locals and multi-statement bodies.
  Source control is expressed as `state` blocks and guarded `to` transitions,
  not `if`/`while`; the compiler lowers those edges to Alpha jumps.
- **Stack-overflow guard** — the sp grows down with no limit check. A higher rung
  proves depth bounds (see `totality_and_bounded_computation.md`); until then it is
  an unchecked region.

## Why this is the Beta-the-language foundation

A "procedure with parameters and locals" is exactly a source construct that lowers
to: prologue, argument moves, body, epilogue. Beta's self-hosting compiler emits
these frames mechanically. Gamma's canonical interpreter and type checker are
Beta programs, so everything higher can grow without another hand-written
assembly compiler.
