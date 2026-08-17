# Canonical terminal semantic-ledger spike

This directory is the bounded Gamma feasibility experiment required by Q7's
`PCC-CANONICAL-SEMANTIC-LEDGER` ruling. It is not the production generator and
does not replace the explicit migration trust graph. It establishes that the
authoritative endpoint can begin at canonical terminal-Psi bytes and remain a
small, typed, interpreter-defined program rather than another Rust verifier.

## Trust boundary and inputs

`psi-terminal-codec/tests/ledger_spike.rs` constructs two ordinary current
`TerminalModule` values and pins their exact `PSITERM\0` v11 encodings. The Rust
fixture builder and `bytes_to_gamma.py` are untrusted test transport. The typed
Gamma program begins with the reusable, PSITERM-neutral primitives in
`../canonical-bytes/`; the spike-specific decoder then:

1. consumes the exact byte list;
2. validates the current marker, closed tags/counts/types, and the exact spike
   shape needed by the bounded ledger;
3. rejects truncation, trailing bytes, unknown tags, invalid identities, wrong
   types, wrong section counts, and shape drift;
4. denotes every retained operation directly; and
5. emits and audits one ranked 22-row ledger.

The transport packs at most seven bytes into one positive Gamma `Int` solely to
keep the source parser shallow. Typed `unpack_bytes` reconstructs every byte
before `decode_module` sees it. No packed integer is interpreted as terminal
semantics.

## Covered Q7 cases

The fixture covers exact and wrapping i8 add, exact and wrapping signed
divide/remainder, toward-zero negative division/remainder, and the signed
`MIN / -1` policy distinction. The ledger audit pins the exact arithmetic
denotations and five safety rows rather than merely counting them.

Its diamond control flow introduces two branch-local premises, invalidates them
at the join, carries the matching value through both predecessor edges, and
creates one merge row that depends on both edge rows. The asymmetric fixture
changes only the false-arm value and must reject. Every prerequisite rank is
strictly lower than its consumer; the positive branch premise is available in
the true block and unavailable at the join.

The call carries two obligations for two canonically ordered callee
requirements. The ledger audits exact enumeration and capture-free positional
substitution:

- obligation 105: `caller.value11 <= 127`;
- obligation 106: `-128 <= caller.value10`.

## Measured result

Measured on Darwin 25.5 arm64, Apple M4 Pro, after building the Beta programs;
these are feasibility observations, not performance promises:

| Item | Result |
| --- | ---: |
| Canonical fixture bytes | 1,026 |
| Assembled typed Gamma core | 1,337 lines / 51,661 bytes |
| Shared canonical-byte layer | 36 lines / 1,373 bytes / 6 functions |
| Spike-specific typed core | 1,301 lines / 50,288 bytes / 136 functions |
| Closed data declarations | 47 |
| Typed functions, assembled | 142 |
| Maximum source nesting | 20 |
| Canonical ledger | 22 rows / 1,546 modeled bytes |
| Prospective reconstruction certificate | 1,352 modeled bytes |
| Beta-written typechecker | 0.09 s / 1,949,696 B max RSS |
| Beta-written reference interpreter | 0.25 s / 3,031,040 B max RSS |
| Independent Python evaluator | 0.03 s / 10,944,512 B max RSS |

The certificate estimate is deliberately explicit: 32 bytes of header, then
44 bytes per row plus one 32-byte reference per prerequisite. It is a sizing
model, not an accepted certificate format.

The first literal transport spelling used one constructor per byte and exposed
a parser-stack failure. That did not require weakening the endpoint: a typed,
semantics-free unpacker made the exact same bytes practical. More importantly,
the original ledger orchestration was rejected by the Gamma checker until it
was decomposed into accessors, validators, row emitters, and sequencing helpers.
That is useful evidence for the ruling: the low implementation should grow as
small modules and closed row tables, not as a second monolithic verifier.

The PSITERM-neutral byte cursor and checked `u8`/little-endian `u16`/`u32`
primitives are now factored into `../canonical-bytes/` and have an independent
typed/interpreter gate. The largest remaining audit tax is mechanical
repetition: Gamma currently has no parametric result type, so the bounded
decoder still declares a result ADT for each parsed semantic type. The full
decoder should measure that remaining cost before making any explicit Gamma
rung correction. The spike itself is cleanly expressible and therefore finds
no language-design blocker.

## Gate

Run:

```sh
sh compiler/gamma/test-canonical-bytes.sh
sh compiler/gamma/test-terminal-ledger-spike.sh
cargo test -p psi-terminal-codec --test ledger_spike
```

The first command checks the shared byte layer independently. The second
typechecks the exact assembled spike source, mechanically erases its
annotations, evaluates matching/asymmetric/malformed fixtures with both the
canonical Beta interpreter and the independent Python evaluator, and pins the
ledger/certificate measurements.
