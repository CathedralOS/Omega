# Constructing checked policy inputs for public_values

The compiler spent 33.778 s evaluating demanded values while building the original filesystem fixture on a cloud Xeon. Reusing each exact reflected input and sharing checked constructors for identical inactive rows reduced that phase to 23.578 s. A separate probe-free release build took 60.817 s end to end, compared with 71.775 s at the shared base. A Ryzen 9 7950X measurement is still required.

## Measured code and host

- Shared baseline and main at acquisition: `e6477bc08c36684b78cca182ba91bd012b4c5a4d`. A later fetch at `1fe6c569a5d35b9e6a5e8f71820136d93a91555f` showed no changes to this policy path.
- Candidate: `d5ac45bd29303ecc6efdf78abf357597d7cf98f6`. Both binaries used release optimization and temporary diagnostic probes. The candidate also counted constructor bodies and input materializations.
- Linux x86-64, Intel Xeon Platinum 8573C, four-CPU cgroup quota. One run per revision per condition. The results measure compiler and dependency cold starts. The OS file cache was uncontrolled.

| Input and prior state | Base phase | Candidate phase | Base wall | Candidate wall |
| --- | ---: | ---: | ---: | ---: |
| Original, empty caches | 33.778 s | 23.578 s | 73.232 s | 69.508 s |
| Source comment edit, warm dependencies | 31.943 s | 23.356 s | 70.957 s | 62.726 s |

With all probes removed, a second release-built pair took **71.775 s → 60.817 s cold** and 71.817 s → 60.935 s after the edit. These runs used the same entry command, cache conditions and original inputs and emitted the same executable.

The phase covers the product invocation of `evaluate_demanded_values`. Wall time includes launching the release CLI and waiting for it to exit. Each build compiled 118 sources. All four builds emitted the same 196,608-byte PE (SHA-256 `57893c489c80f4269cd46352bd61dc96b78a5a354d30d60c1e67050c673f0d1a`) and the same two warnings.

## Work the checker sees

| Policy caller | Base entry operations | Candidate entry operations | All operations in candidate checked closure |
| --- | ---: | ---: | ---: |
| Two 13-field layouts, each | 9,771 | 230 | 415 |
| 2-field layout | 9,771 | 153 | 624 |
| 18-field layout | 9,771 | 265 | 736 |
| Three access policies | 3 each | 3 each | 25, 25, 37 |

Count operations in all blocks of the accepted entry machine. The last column includes generated constructors and the policy bodies. Each layout has three constructor bodies containing 8, 71 and 8 operations. The four entry counts sum to 878, down from 39,084. The compiler still supplies each array slot as an ordered operand. Each constructor returns an owned value. The compiler builds four full layout argument records and three access reflection graphs for the fixture. Equal layout inputs share planning and evaluation storage only after comparing reflected fields, retirements and target facts. Access policies retain their prepared input through evaluation in the same immutable round. Returned plans still undergo their ordinary checks.

## Running the measurement

Build each revision with `mbx build --release -p omega`. Take `source/library` and `source-rust/application/tests/fixtures/filesystem/run/public_values` from that revision using `git archive`. Set F to the absolute extracted fixture directory and LIB to the extracted source/library. Start in a fresh working directory with private, empty HOME, XDG_CACHE_HOME, TMPDIR and OUT directories. Time the process below through exit.

```sh
OMEGA_LIBRARY_ROOT="$LIB" "$OMEGA" --timings --report-file "$REPORT" \
  --target windows_x86_64 --build-dir "$OUT" "$F/build.omg" </dev/null
```

For the second run append a comment to the extracted main.omg and repeat with the same paths and caches. For policy identity comparison use the identical source root across compiler revisions. Original build.omg SHA-256 is `dc4bda0f879ed834eb3f9ff081d6a0b00e483fd86bebe8c3e0c3f29e5d8ca7fd` and main.omg SHA-256 is `85df47708700d685cf77bae0fed481298d00ab8808885de9ae63e2e78cab9fa8`.

An authored policy read alignment one from the last field and the last case's last payload field. The negative compiler test changed every inactive repeated field to alignment zero and the normal proof check refused the input. Returning inactive key zero as a live placement independently produced `layout refers to unknown field key 0`.

All four returned layout plans and their private placements matched byte for byte across revisions. With both compilers pointed at the same source root, all three access results also matched, including their authority and issuance identities. SHA-256 of the seven sorted, newline-terminated diagnostic records was `44c716e355e8cfc501ef2df45b957ce683341cf22e9c8d7b23fa76aea6e61c43`. Moving the source root changes the package identity in access results. All 105 release source-compiler library tests passed, including positive and negative access policy tests. Both revisions ran the original fixture with Linux native/Psi agreement at exit 0. The Windows executable exited 0 under Debian Wine 10. Windows-target Psi on Linux reported a missing trusted Windows filesystem write binding (exit 200).

For the hardware gate, run the original fixture with a release-built compiler and empty compiler, dependency and output caches on the Ryzen 9 7950X. Time the real CLI through exit and launch the emitted executable on Windows. Record the revision, exact command, wall time, exit, output hash and host. Measure the edited-source case separately.
