# Binary64 dot, scaled norm and AXPY

Two complete applications call the owned `numerics::linalg::dot_scaled_norm_axpy`
slice API. `ordinary/` exercises ordered dot, scaled norm, disjoint AXPY,
exact self-AXPY, empty inputs and length mismatch without mutation. `scaled/`
exercises large/small representable norms whose naive squared norms overflow
and underflow, zero vectors, dot overflow and late AXPY failure without partial
mutation. Both return normally with exit 0 on every authored assertion;
a failed assertion crashes `Trap`. Neither program prints output.

`invalid_alias/` passes shared and exclusive views of one array into ordinary
AXPY. Expected source rejection is overlapping loans, check exit 1. A storage,
package-import or numeric diagnostic does not count. Use `try_axpy_self_f64`
and one exclusive loan for permitted exact self-aliasing.

## Frozen answers

[`oracles.json`](oracles.json) predates execution. Ordinary answers follow
3*1+4*2=11, sqrt(3²+4²)=5, 2*(3,4)+(1,2)=(7,10), then
2*(7,10)+(7,10)=(21,30). Scaled answers use the same 3–4–5 identity at
decimal scales 1e200 and 1e-200. Relative tolerance stays 1e-14; zero and
integer-valued results require exact equality. Failed AXPY uses [1,4e200] with
alpha=1e200: its first result is finite and its second overflows, challenging
the no-partial-mutation guarantee.

## Current-source execution

Build from the candidate source, not an inherited executable:

```sh
CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p omega </dev/null
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
target/debug/omega --check samples/cli/scientific/w100/nf09_dot_scaled_norm_axpy/ordinary/main.omg </dev/null
target/debug/omega run samples/cli/scientific/w100/nf09_dot_scaled_norm_axpy/ordinary/main.omg </dev/null
target/debug/omega run --both samples/cli/scientific/w100/nf09_dot_scaled_norm_axpy/ordinary/main.omg </dev/null
target/debug/omega --check samples/cli/scientific/w100/nf09_dot_scaled_norm_axpy/scaled/main.omg </dev/null
target/debug/omega run samples/cli/scientific/w100/nf09_dot_scaled_norm_axpy/scaled/main.omg </dev/null
target/debug/omega run --both samples/cli/scientific/w100/nf09_dot_scaled_norm_axpy/scaled/main.omg </dev/null
target/debug/omega --check samples/cli/scientific/w100/nf09_dot_scaled_norm_axpy/invalid_alias/main.omg </dev/null
```

Consumers import individual declarations: a module import does not flatten
them. Both applications import core `float_operations` so comparisons select
the exact source requirements/providers. IEEE comparison is not structural
proof equality.

`run --both` qualifies Terminal termination only when it reports execution and
agreement, not `DECLINED`. It does not compare program streams. Native and
Terminal stdout/stderr need independent observation; matching exits cannot
stand in for output evidence. A requested review, denial or custody hold remains
a refusal, not permission to accept policy or reuse old locks.

## Provenance and scope

These paths were recovered from candidate
`71094a66003f97c81951218c3b7384f3adf449c3`, then adapted to current explicit
numerics imports and format-qualified float operations. Original answers and
the rejection control remain intact. That candidate's historical attempts were
unqualified and supply no current approval, artifact or execution evidence;
they are retained in the recovery record, not copied into the shipped API.

This contiguous-slice API uses explicit backing loans, no allocation, copying,
runtime address comparison or hidden scratch storage. Rank-two/strided-vector
adapters and accuracy-policy integration remain separate unfinished interfaces;
these programs neither invent nor claim those adapters.
