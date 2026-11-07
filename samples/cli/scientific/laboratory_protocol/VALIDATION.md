# IND22 validation evidence

The complete application passed on Linux x86-64. Native stdout matched every
frozen oracle byte-for-byte, with tolerance zero; the public Terminal comparison
explicitly agreed on every exit. The complete intended-invalid application was
rejected at its pinned call requirement. No compiler repair was needed.

## Revision and executor

- Main base: `0f68686041ad078185e34fb65f2e5fa9d96e05dc`.
- Oracle checkpoint, before any Omega execution: `a77bfe560d93d95c2dc1bb56728c3a77549c5721`.
- Executed source checkpoint: `538610e02b707d682e8e295e8062a35848546171`.
- Host: Linux 6.18.44, x86-64, glibc 2.41; available CPUs: 5.
- Rust: pinned `nightly-2026-09-04`, release profile, 4 build jobs, one build.
- Target: `linux_x86_64`; one Omega execution at a time.
- Compiler SHA256: `c54b7f3f7137c5ed54706db5e1b53cec9dbab834c4d8dc2644b09f9b0d7de225`.
- Cargo target: `/workspace/cloud-setup/cache/mbx/targets/v1/bd37e27070a7008a93218d75eb8ba6218ffc4af31bd0c4b9a0608be4d4803db8`.
- Compiler: the target's `release/omega`, built from the base's unchanged Rust/library source in this isolated worktree.
- Nominal native ELF SHA256: `243cf517255cfe9a65e557ed17ca2bb4155e0a1783998afab7d5be10a6063568`.
- Oracle manifest SHA256: `01a9156d3401ae79747f04f4b32eece829038b14c48c124053773b129055ebe3`.

## Commands and results

From the candidate repository root:

```sh
source /workspace/cloud-setup/activate.sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
mbx build --release -p omega
./target/release/omega update --project samples/cli/independent/laboratory_protocol --target linux_x86_64
# Inspect and accept the local application and bundled std review decisions.
./target/release/omega update --resume --project samples/cli/independent/laboratory_protocol
python3 samples/cli/independent/laboratory_protocol/validate.py --omega ./target/release/omega --timeout 600
```

The compiler build exited 0 in 11m21s. The supported per-command escalated
compiler context was approved and used after a default-sandbox update rejected
synthetic root ancestry with `cache custody ancestry is owned by an unrelated
user`. UID 1000 and real custody checks were retained; no ownership, guard or
permission configuration was changed.

The first package checks exposed authoring errors, all corrected: relative std
paths, explicit named-module selections, explicit `Saturating` conversions, and
`Report::row`'s published `invokes console;` ceiling. Initial review covered only
the assigned local application and bundled local std, including 4 application
Console decisions and 282 std decisions. This acceptance does not claim a full
standalone std source audit. After adding the negative-control oracle and fixing
the validator, a check rejected stale accepted source identity; normal `omega
update` refreshed the two changed support files with no capability changes.

The final validator exited 0 with no failures. It ran these exact operations
serially (absolute paths in `build/validation/record.json`):

```sh
python3 samples/cli/independent/laboratory_protocol/reference.py
./target/release/omega --check --target linux_x86_64 samples/cli/independent/laboratory_protocol/main.omg
# Once for every fixture below, with that fixture's bytes on stdin:
./target/release/omega run --both --keep samples/cli/independent/laboratory_protocol/main.omg < samples/cli/independent/laboratory_protocol/fixtures/nominal.txt
./target/release/omega --check --target linux_x86_64 samples/cli/independent/laboratory_protocol/invalid/main.omg
```

| Fixture | Native exit | Exact stdout | Terminal exit |
| --- | --- | --- | --- |
| budget | 65 | matched | 65 |
| cancellation | 0 | matched | 0 |
| conflict | 0 | matched | 0 |
| eof | 0 | matched | 0 |
| framing | 0 | matched | 0 |
| invalid_input | 0 | matched | 0 |
| nominal | 0 | matched | 0 |
| repeated | 0 | matched | 0 |
| timeout | 0 | matched | 0 |

The intended-invalid check exited 1 with:

```text
cannot prove requires contract for call release from Main::main: lease.owner == 2
```

It also explained the missing `lease.owner == actor` fact. The rejection matched
`invalid/expected.txt`, which was pinned before its first execution.

## Artifacts and boundaries

`build/validation/record.json` contains commands, statuses, timings, binary and
source hashes, oracle identity and the empty failure list. Each operation has
retained `.stdout` and `.stderr`. Native artifacts are retained at the directories
named by each run's stderr; the Library delivery includes the nominal ELF and
all recorded output observations. Frozen expectations are tracked in `fixtures/`.
The Library delivery includes a local Git bundle and binary-safe patch based on
the exact main base; neither requires a remote worker branch.

The checks challenge retained abort ownership, wrong-actor actions, repeated
commands, exact and overshoot deadlines, denied-command atomicity, immediate
lid-open abort, explicit recovery, full-line draining, EOF framing and the
64-command limit. Other targets are declared, but native execution is claimed
only for Linux x86-64. Current public `run --both` compares exit status and
forwards native stdout; Terminal stdout equality is not claimed.

The final evidence documentation does not change any executed Omega source or
oracle bytes. Before delivery, refresh the package's normal accepted identity
for those documentation changes and check the final candidate again.

## Executed Omega source hashes

- `build.omg`: `21d2d2215fff8dc50d88953b21eb5700b2b4f54c9340f843b9d3ec49e2873925`
- `invalid/build.omg`: `9ff96573ace8c30115c57f09b367627efad81766028d27cb13a648c91e4236a2`
- `invalid/main.omg`: `fa1bcae4a2b103e31c0d06c8d2d56f407a3cbb5da54b1c0a01a82aa475c90424`
- `main.omg`: `c5f8e471cc13fc99f6454f02186d25e0bc797d2a45f410ee526b660d35b75e8f`
- `protocol/model.omg`: `2fc2c5c3db3c9d628b2e8cecc3735a634d1ce190d96e0a22be30ae514985de52`
- `protocol/report.omg`: `0d470511825a427ab25414c59d3d3b5ebf32aa5dc0ed9f481f0702a300f8de2a`
