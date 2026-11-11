# Chapter 22: Inline assembly

Use assembly when you need a specific instruction. Omega assembly is parsed
and checked: every supported instruction has a contract, and the compiler
verifies your code against it like any other operation.

## A block inside a machine

```omega
asm {
    out port, value
}
```

Operands are ordinary typed places. Here `port` must be a `u16` and `value` a
`u8`; nothing narrows implicitly. Separate several instructions with `;`.

A block can state what it needs, what it changes and what it establishes:

```omega
asm
    requires ready
    clobbers none
    ensures ready
{
    lfence; sfence
}
```

A `clobbers` list must be exact. Unknown instructions and raw bytes are
rejected.

## Values between blocks

Registers and flags do not survive from one block to the next. Pass values
through variables:

```omega
asm { mov saved, input }
asm { mov output, saved }
```

## A whole machine in assembly

`asm machine` gives a machine an assembly body. Nested `asm machine`
declarations are its basic blocks:

```omega
asm machine drain(count: u32)
terminates
registers ecx = count
{
    transition -> check;

    asm machine check {
        test ecx, ecx;
        jz done;
        transition -> step;

        asm machine step
        requires ecx > 0
        {
            sub ecx, 1;
            jmp check;
        }
    }

    asm machine done
    requires ecx == 0
    {
        ret;
    }
}
```

The compiler follows the control flow: `jz done` establishes `ecx == 0` on the
way to `done` and `ecx > 0` on the way to `step`, which is what lets `sub`
pass. `transition -> step;` after the conditional jump names the fall-through
block. `registers ecx = count` loads working registers once on entry.

An assembly machine can be used anywhere an ordinary one can, including as a
callback to a generic library machine.

## Assembly has no extra privileges

- **Authority.** A machine containing `wrmsr` still needs the authority to
  write that register, exactly as if it had called a wrapper.
- **Memory.** A memory operand needs the same bounds, alignment and access
  proofs as an indexing expression. An integer in a register is not a pointer
  you may dereference.
- **Calling convention.** An assembly machine that implements a boundary
  requirement follows that requirement's
  [calling plan](../spec/build/calling_plans.md). At `ret`, the compiler
  checks the result is where the plan says and that preserved registers are
  preserved.

## Under the interpreter

The interpreter runs each instruction's meaning, not native bytes. A `mov` is
an assignment; a port write goes to the environment the host supplies. An
instruction the interpreter has no meaning for cannot run there.

## Where next

[Chapter 23](chapter_23_testing.md) covers testing. The supported
instructions are in the
[instruction catalog](../../omega-rust/psi/foundation/language-core/inline_assembly.md),
and the complete rules, including interrupts and mode switches, are in
[assembly](../spec/language/assembly.md).
