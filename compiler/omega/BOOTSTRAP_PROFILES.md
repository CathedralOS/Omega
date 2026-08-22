# Omega0 bootstrap profiles

This file freezes the first executable contracts for the Delta-built Omega
compiler while the work remains in its historical `compiler/` placement. It
moves with that work to `bootstrap/omega0/compiler/`; it is not a product Omega
language specification.

Two profile versions are intentionally tracked separately:

- **Delta/Omega0 implementation profile D0** is the language in which Omega0
  must be written. It is frozen now so the compiler does not acquire facilities
  merely because the Rust on-ramp has them.
- **Omega canary acceptance profile O0** is the first input Omega0 must accept.
  It is frozen now as a vertical proof of the pipeline, not falsely presented as
  sufficient to express the future production compiler.

The production-self-host acceptance profile remains open until the production
compiler has an Omega source tree. It will be the smallest monotonic extension
of O0 that accepts that exact source. Sufficiency cannot be established from the
current Rust product implementation.

## D0 — Delta implementation profile for Omega0

Omega0 source may use only the following already implemented, self-hosted Delta
surface:

- `boundary trait`, `data`, free `machine`, and `Type::machine(&mut self, ...)`
  declarations;
- `i32` scalars; fixed `[i32; N]` and `[u8; N]` backing fields; zero-initialized
  receiver storage;
- locals, mutable locals and fields, checked indexed field access, and integer
  offsets as durable handles;
- `+ - * / %`, comparisons, bit operations, shifts, `min`/`max`, and
  parenthesized expressions with trapping arithmetic unless an explicit domain
  says otherwise;
- state-machine control flow, state parameters, calls/recursion, returns, and
  tag/payload `case` data where useful;
- `read_byte`, `write_byte`, `write_line`, and `exit_process` as the explicit
  compiler boundary;
- the fixed-backing allocation convention below.

The calling-profile limits are at most four value parameters for a free machine
and at most three value parameters for a self method. Omega0 must use checked
failure before exceeding any source, table, arena, call, or target offset bound.
It must not discard input or declarations to remain within a bound.

The following are outside D0: host pointers, ambient heap allocation, individual
`free`, garbage collection, dynamic locals, threads, atomics, modules, a
production `PagedArena`, and optimization-specific data structures. Source is a
single deterministic byte stream. Multi-file input will be supplied by a
separately audited bundling contract until native packages are implemented. The
version-1 contract is frozen in [`OMEGA0_BUNDLE.md`](OMEGA0_BUNDLE.md).

### Fixed-backing allocation convention

A backing array is provisioned as zeroed storage by the executable loader.
Logical allocations have runtime sizes and return integer offsets into that
backing:

```text
reserve(count, alignment)
  valid arguments require count >= 0 and alignment > 0
  for valid arguments, pad = (alignment - (cursor % alignment)) % alignment

  success, when pad <= capacity - cursor and
                count <= capacity - (cursor + pad):
    base = cursor + pad
    cursor = base + count
    ok = 1

  failure, including invalid arguments or exhaustion:
    cursor and backing are unchanged
    ok = 0

reset(mark)
  success only when 0 <= mark <= cursor; set cursor = mark
  otherwise leave cursor unchanged and report failure
```

Zero-sized allocations succeed at the aligned cursor. Successful allocations
are deterministic, monotonic, aligned, and disjoint until a valid bulk reset.
Handles are indices, not host addresses. The executable canary is
[`../delta-rs/samples/bootstrap-storage.alp`](../delta-rs/samples/bootstrap-storage.alp).

## O0 — Omega vertical-canary acceptance profile

O0 is exactly the single-file console program shape represented by
[`../lattice-corpus/cli_mvp/main.omg`](../lattice-corpus/cli_mvp/main.omg):

- UTF-8 source with whitespace, line comments, identifiers, integer literals,
  string literals, and the punctuation used by the declarations below;
- `use omega::language::std::console;`;
- one `data Main` declaration with one `console: Console` field;
- one `machine Main::main(&mut self)` entry;
- `self.console.write_line(<string literal>);` followed by
  `self.console.exit_process(<i32 literal>);`.

The O0 front end must lex and parse the complete input, reject trailing or
unknown constructs, resolve `Main`, `Console`, `main`, and both console
operations, and type-check their receiver and arguments. Duplicate declarations,
unknown names, wrong receiver types, wrong argument types/counts, malformed
strings, and a missing entry are negative gates.

Accepted O0 lowers through the selected terminal-Psi representation to a
deterministic runnable artifact. The artifact must print the literal plus one
newline, then exit with the requested low-byte status; those observations must
agree with canonical meaning.

Terminal-Psi vocabulary 23 now represents the scalar half of that lowering.
`BoundaryMachineDeclaration` carries ordered scalar parameter types and
`OperationKind::BoundaryCall` carries ordered scalar `ValueId` arguments. The
checked producer retains exact scalar expressions, the codec and verifier bind
their order, the interpreter supplies them to the effect handler, and Omega's
abstract operation preserves them. Provider candidates remain outside this
scalar boundary slice and reject such signatures rather than silently ignoring
them.

O0 still waits on a genuine native realization of `exit_process(i32)`. Omega
target lowering deliberately rejects every nonempty scalar boundary call until
that realization exists; its metadata-only port settlement is not an exit
operation. Treating the exit literal as an unrelated machine return or
introducing an Omega0-only IR would evade, not close, the intended seam. The
string passed to `write_line` must likewise retain its exact structural carrier
and custody through the canonical call.

O0 excludes build files, packages beyond the fixed `use`, arbitrary data,
general expressions, user calls, control flow, allocation, proofs, and
optimization. Those features enter later numbered acceptance profiles only when
required by the Omega-source production compiler or a deliberate conformance
slice.
