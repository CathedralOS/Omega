# NF20 assignment and evidence

Assignment: scalar/vector-compatible fixed-step RK4 interface, static RHS and
explicit context, step budget and caller-provided bounded traces. Parent owns
independent review and normal landing. Dependencies: NF01 package/descriptors,
NF02 finite-value/accuracy policy, NF09 vector copying/AXPY. The scalar kernel,
two distinct full sample projects and separate invalid-source project are
implemented; shared-vector execution remains pending the exact dependency
signatures listed in the API note. Finite inputs/stages are a caller obligation
until the NF02 adapter is available. This candidate is not full-wave acceptance.

Started 2026-10-10, UTC. Starting checkout `/workspace/Omega`, branch `work`,
revision `6e4f0c37ecd6dbb26d228b3045d40bc6b243d0bb`, clean. Fetched main:
`26c92c17b8f4b21f39e14a5be57c85cfdec601de`. Isolated branch `nf20-rk4`,
worktree `/workspace/Omega/.codex/worktrees/nf20`, based on fetched main.
Read canonical AGENTS.md, `.agents/skills/advance/SKILL.md`, and relevant
validation policy in `tools/testing.md`; instructions unchanged between bases.
No numerics library, scientific sample directory or NF01/NF02/NF09 interfaces
existed on inspected main. Ownership check `python tools/claims.py status`
returned coordination deadline exceeded, with no successful registration.
Claim command (from starting checkout) rejected because NF20 was absent from
TASKS.md. No board aliases or competing parent claims were created.

Host: Linux x86_64, `nproc`=5, cgroup `cpu.max`=`400000 100000` (4-core quota),
`memory.max`=17179869184 bytes (16 GiB). `free -m` before build: total 18055,
available 17354 MiB. Initial free disk about 30 GiB; measured during build:
31,177,613,312 bytes available on 33,770,192,896-byte overlay filesystem.

Frozen fixture qualification: `python
samples/cli/scientific/w100/nf20_fixed_step_rk4_interface/oracle.py`, exit 0.
80-digit decimal classical RK4 compared against analytic exponential formulas:
decay error 1.4697591442852953e-8; forced growth error 1.7332378336029875e-8.
Both fit the previously frozen absolute tolerance 2e-8, relative 0. The pending
coupled-vector oracle is also frozen in oracles.json. Python syntax compilation
of prepare/oracle/verify exited 0. These are independent fixture checks, not
Omega execution evidence.

Build setup: initial mbx invocation failed because rustup's default home was
read-only. Setting RUSTUP_HOME=/workspace/.rustup and CARGO_HOME=/workspace/.cargo
selected installed pinned toolchain; mbx then failed because default cache
/home/agent/.cache was read-only. XDG_CACHE_HOME=/workspace/.cache is the normal
supported Linux cache setting; no ownership, permissions or custody guards were
modified. The build completed with exit 0 in 4m 24s (dev profile, unoptimized; existing compiler warnings retained in /tmp/nf20-build.log; summary in evidence/build-summary.txt). Exact command:

```sh
RUSTUP_HOME=/workspace/.rustup CARGO_HOME=/workspace/.cargo XDG_CACHE_HOME=/workspace/.cache PATH=/workspace/.cargo/bin:$PATH mbx build -p omega > /tmp/nf20-build.log 2>&1
```

One Rust build and one Omega execution at a time on this executor.

## Omega route results

Executed after freezing source and fixture oracles:

```sh
XDG_CACHE_HOME=/workspace/.cache python samples/cli/scientific/w100/nf20_fixed_step_rk4_interface/verify.py --omega target/debug/omega --out /tmp/nf20-verification-final
```

Runner exit 1. Seven sequential executions, all environmentally blocked before
source validation. Both positive projects: check exit 1, native exit 200,
Terminal inspection exit 1. Invalid-source check exit 1, with intended signature
rejection **unconfirmed**. Every route reports exactly:

> cannot open private source resolver storage: git cache entry `/` is invalid: cache custody ancestry is owned by an unrelated user

Native routes wrap it in `native compile FAILED`. No source/check/native/Terminal
or rejection success is claimed. The platform cache environment remains a parent
investigation; system ownership, permissions and custody checks were untouched.
Full logs, exact argv/timing/exit JSON and source snapshots are in `evidence/`.
They retain the same kernel SHA256
`9b5d2d666eed548642aaa6fc484bbc8c6ce83e9fe277bfa713971ab867948c30`.

Next acceptance: restore legitimate cache custody configuration, rerun the
seven-route command, resolve any source errors revealed by that run, confirm the
bool RHS signature rejection, then integrate NF01/NF02/NF09 signatures and
qualify the coupled-vector fixture. The scalar source remains unvalidated by
Omega until the environment admits the resolver. Do not land as full acceptance.
