# The Alpha calling convention

> The foundation of Gamma-the-language. This is the one thing that turns "an
> assembler" into "a language you can write a compiler in." The Beta assembler
> itself does not use it: every value that must survive a
> `call` lives at a *fixed global
> address* (see the `524288+` block in
> [`assembler.beta`](../beta/compiler/assembler.beta)), so there are no locals,
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

The D23 Gamma compiler source emits this `AlphaBootstrapV2` physical layout:

```text
[0, 1048572)                Alpha tape payload
[1048572, 1048576)          reserved raw-tape tail
[1048576, 2097152)          guarded downward generated data stack
[2097152, 4194304)          reserved separation
[4194304, 138412032)        biased 128 MiB source-visible raw memory
[138412032, 268435456)      hidden-return-stack allowance
```

The rebuilt compiler artifact, seeds, checker, downstream emitters, and gates
all use this profile; no mixed V1/V2 construction route remains current.

Every emitted byte/word access checks the logical 128 MiB bound, rejects a
signed-negative Word address, and adds the raw base before touching Alpha
memory. Thus logical address zero is initially zero rather than tape byte zero.
Every generated frame and expression reservation subtracts first, then checks
`r15 >= 1048576` before any generated memory access. Failure terminates inside
the current procedure with status 250 and no further store. Because the
canonical compiler emits at least one guarded 8-byte frame word per active
procedure, the 1,048,576-byte data region bounds semantic depth. At the failing
edge, including one transient shared-memory-guard call, the hidden Alpha return
stack remains at or above physical byte 267,386,872—well above the raw-memory
ceiling 138,412,032. Thus neither generated stack can alias tape or raw memory.

## Register roles

| Register | Role |
| --- | --- |
| `r15` | **stack pointer (sp)** — an offset into memory, grows **downward**. Initialized once at startup. Doubles as the expression-evaluation stack. |
| `r14` | **frame pointer (fp)** — fixed for a procedure's lifetime; params/locals live at `[fp - 8 - 8*slot]`, so they stay addressable while `sp` moves for expression temporaries. The compiler uses it; hand-written code may skip it (the examples below do, tracking offsets by hand). |
| `r0`–`r3` | Gamma v1's complete four-register **argument** surface; no further arguments are admitted. |
| `r0` | the **return value**. |
| `r0`–`r5` | **caller-saved** scratch. A caller that needs one of these to survive a `call` saves it in its own frame first. |
| `r6`–`r12` | **callee-saved**. A callee that uses one must save it on entry and restore it before `ret`. |
| `r13` | **word-size constant**. Generated-program startup sets it to `8`; generated procedures preserve it and shared frame/stack macros consume it directly. |

(This supersedes delta's current "vars `a`–`j` live in fixed registers `r6`–`r15`"
scheme: under a real convention, locals live in **frames**, not fixed registers,
and `r15` is reserved as the dsp.)

## Frame protocol

The compiler brackets each procedure with an **fp-based** prologue and epilogue
(schematic; immediates are loaded through a scratch register; the canonical
compiler emits these encodings directly into Alpha tape):

```
proc:
        ; prologue
        sub   r15, r13              ; reserve caller-fp word (`r13 == 8`)
        imm   r4, 1048576
        jlt   r15, r4, fault         ; fail before any access below the stack
        store r15, r14
        mov   r14, r15              ; fp = sp   (the frame base)
        sub   r15, framesize        ; allocate params + locals below fp
        imm   r4, 1048576
        jlt   r15, r4, fault
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

The canonical compiler deliberately emits the minimum frame word even for a
leaf, because that invariant bounds both stacks. A later verified optimization
may elide it only while preserving an equivalent call-depth bound. **Spilling
across a call**: a caller-saved value
(e.g. an argument in `r0`) that must outlive a `call` is stored into the frame
before the call and reloaded after.

## Proven on the seed

Three hand-written examples exercise the convention end-to-end (assembled by
the Beta assembler, then run on the Alpha seed):

- [`factorial.beta`](../../tests/beta/compiler/examples/factorial.beta) — single recursion;
  `factorial(5)` exits **120**. The frame holds one slot: `n`, saved across the
  recursive call.
- [`fib.beta`](../../tests/beta/compiler/examples/fib.beta) — **tree** recursion (two recursive
  calls per frame); `fib(10)` exits **55**. The frame holds two slots and never
  relies on a register surviving a call.
- [`gcd.beta`](../../tests/beta/compiler/examples/gcd.beta) — **two parameters** (`r0`, `r1`) and
  a **tail call that needs no frame**; `gcd(48, 36)` exits **12**. Shows the
  leaf/tail case where a procedure skips the frame entirely.

Build one through `tools/bootstrap/beta/build.sh`, for example with
`tests/beta/compiler/examples/factorial.beta`.

## Remaining limits

- **Argument count** — Gamma v1 accepts at most four arguments. Calls and
  procedures with five or more are rejected; stack-passed arguments are not a
  deferred compatibility path.
- **Language control shape** — Gamma now has locals and multi-statement bodies.
  Source control is expressed as `state` blocks and guarded `to` transitions,
  not `if`/`while`; the compiler lowers those edges to Alpha jumps.
- **Static depth proofs** — higher rungs may still prove that status 250 is
  unreachable for a program, but memory containment does not depend on such a
  proof.

## Why this is the Gamma-the-language foundation

A "procedure with parameters and locals" is exactly a source construct that lowers
to: prologue, argument moves, body, epilogue. Gamma's compiler emits these frames
mechanically. Delta's current interpreter and type-checker components are Gamma
programs. The canonical next edge remains a Gamma-written Delta compiler that
emits Alpha tape directly; these components must be absorbed into it or reduced
to bounded oracles.
