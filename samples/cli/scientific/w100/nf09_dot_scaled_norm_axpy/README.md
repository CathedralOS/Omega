# NF09 dot, scaled norm and axpy

Two complete application projects call the single owned library implementation.
`ordinary/` exercises geometry, ordered dot, disjoint axpy, permitted exact
self axpy, empty inputs and length mismatch without mutation. `scaled/` exercises
large and small representable norms whose naive squared norms respectively
overflow and underflow, zero vectors, dot overflow, and late axpy failure without
partial mutation. Both return normally with exit 0 on every authored assertion;
a failed assertion crashes `Trap`.

`invalid_alias/` is a separate complete invalid-source control. It deliberately
passes shared and exclusive views of the same array into ordinary axpy. Intended
acceptance is source rejection for overlapping loans, compile exit 1. A storage,
package-import or numeric diagnostic does not count. Use the one-loan
`try_axpy_self_f64` spelling for the permitted exact self-alias operation.

The frozen `oracles.json` predates execution. The ordinary answers follow
3*1+4*2=11, sqrt(3^2+4^2)=5, 2*(3,4)+(1,2)=(7,10), then
2*(7,10)+(7,10)=(21,30). Scaled answers follow the same 3–4–5 identity
at decimal scale 1e200 and 1e-200. Relative tolerance is fixed at 1e-14;
zero and integer-valued results require exact equality. The stress project uses
[1,4e200] for failed axpy so its first arithmetic result is finite and its second
overflows, challenging the no-partial-mutation guarantee.

From the candidate worktree, point the compiler at this library and run each
route separately (one Omega process at a time):

```sh
. /workspace/cloud-setup/activate.sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
omega --check samples/cli/scientific/w100/nf09_dot_scaled_norm_axpy/ordinary/main.omg
omega run samples/cli/scientific/w100/nf09_dot_scaled_norm_axpy/ordinary/main.omg
omega run --both samples/cli/scientific/w100/nf09_dot_scaled_norm_axpy/ordinary/main.omg </dev/null
omega --check samples/cli/scientific/w100/nf09_dot_scaled_norm_axpy/scaled/main.omg
omega run samples/cli/scientific/w100/nf09_dot_scaled_norm_axpy/scaled/main.omg
omega run --both samples/cli/scientific/w100/nf09_dot_scaled_norm_axpy/scaled/main.omg </dev/null
omega --check samples/cli/scientific/w100/nf09_dot_scaled_norm_axpy/invalid_alias/main.omg
```

`run --both` qualifies Terminal comparison only if it explicitly reports
agreement; `DECLINED` is not Terminal acceptance. Bare check success does not
qualify native or Terminal execution. If package review is requested, inspect
and accept the intended entry bindings through the normal update/review route.
Do not bypass review or fabricate package metadata owned by NF01.

This candidate has slice kernels, not rank-two descriptor adapters. The `.api.txt`
checkpoint requests concrete NF01/NF02/NF05 signatures; their surfaces are absent
on fetched main `9d4451128dd05885e7b9c591796b2760189851db`. Contiguous slices
are explicit backing loans and neither copy nor allocate. No new descriptor,
policy, package entrance or competing dependency implementation is introduced.

Main-revision build command:

```sh
CARGO_TARGET_DIR=/workspace/Omega/target mbx build -p omega --release
```

The build stopped fetching `crossbeam-deque 0.8.8` with HTTP 403, “No approved
upstream IPv4 address”. Preliminary execution uses the existing unrecorded
`omega 0.1.0-dev (x86_64-unknown-linux-gnu)` binary, not a claimed main build.
Every attempted execution route stops before source validation because private
source storage rejects `/` ancestry owned by an unrelated user. No route has
qualified and invalid-source rejection remains unobserved. Full diagnostics,
commands, statuses, host observations and oracle hashes are in `evidence/`.
System ownership, permissions and custody guards remain untouched.
