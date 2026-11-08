# Cathedral timer boot qualification

Cathedral’s Omega kernel boots through UEFI and wakes ordinary code on eight
advancing timer ticks. A separate observed run verifies eight interrupt returns,
unchanged interrupted registers and return frames, and retained table, context
and stack guards. These results qualify the compiler candidate below on the
pinned single-CPU QEMU/OVMF platform; they do not establish real-hardware support.

The undisturbed serial output ends with:

```text
Mapping checked
GDT checked
TSS checked
Segments ready
........Timer wakes 8
```

All eighteen earlier boot markers appear in order before the wake line,
including `Owned 34 MiB`, `Boot map ready` and `Kernel entered`. Source and native
control-flow review tie those markers to successful memory-map acquisition,
ExitBootServices and transfer of the two existing roots into the kernel.
The VM remains alive at the wake result.

## Exact inputs

| Input | Identity |
| --- | --- |
| Compiler source | `843ab01ddb8adc3d5ecf70d236f57db065af3e4f` |
| Checked customer source/library manifest | `084781435443f0d1c0ab60c07e9c2a8da30a90467422c81b200e74c063eee639` |
| Saved Psi bytes | `82002ffc567e2ae139a83f887e9c39fcea300a9ff2147a449be4d205159259fc` |
| Saved native inputs | `12252a71943db936a2b513f10ea4f6bacbacc485334664cbdec3824faf92e636` |
| Native artifact | `0ec43d39d70deb766cf2cef1a7e0e1787daf7d351a7e8010068f3aee1743c1ae` |
| PE64 image, 17,024,000 bytes | `c7ef3610fb7589ab5f55a5b47fb54f97cff091716303396a0f0598544aa60c56` |
| Two original installation conditions | `d8f522bbf03114abf3017ed823fa4dae6dd9ec85fc1f2e238a8312a35380934a` |

Compilation uses qualified retained inputs. The native phases do not reopen or
recheck source. This qualification precedes updating Cathedral’s ordinary Git
library pins to the published compiler; that final package qualification belongs
to the Cathedral landing.

The platform is QEMU 11.0.2, `pc-q35-11.0`, `qemu64-v1`, TCG, one CPU and
128 MiB, with ordinary UEFI pflash/ESP loading and no network. QEMU SHA256 is
`368d3ff311b708731ec0b43bb337fa27da082e1048d97ae6b0e6872725d9ee44`;
firmware code is `33090cc07675baa5190d9f1e84bf5176b33bcbfa9bacac522961150cdb6dbb2a`;
initial variables are `5d2ac383371b408398accee7ec27c8c09ea5b74a0de0ceea6513388b15be5d1e`.

## What was observed

The undisturbed run completes in 12.220 seconds, with unchanged inputs and ESP
image. Its result SHA256 is
`e7291753406ea840cb0eba1a9cffe77b80b7af62024982662ee050d48e5f92b4`;
serial SHA256 is `14793ddd7c84194c4f9a6b7d441b52734a4233d64bfee4200946f753ce16c9c6`.

The separate stopped run completes in 12.399 seconds. It authenticates its own
new loader occurrence and live entry/Work bytes, rather than accepting the prior
run’s candidate address. The image loads at `0x2737000`, size `0x1435000`,
entry `0x3774039`. The stopped occurrence UUID is
`f66cbfa6-3c4c-4efe-aceb-4bbed233a251`; result SHA256 is
`65e7245ae1781f5fd3eaf53883a9d7f79e64069c11201b5b5dcd394d1d28233b`.

It checks all 33 gates, GDT/TR and encoded TSS; eight entry/pre-IRET/after-IRET
rounds; advancing counters and timestamps; acknowledgement completion; and
interrupted general registers, XMM registers, MXCSR and the five-word return
frames. Seventeen phases each compare seventeen table, task-state and stack
guard words (289 comparisons), with the two context canaries checked separately.
The exact wake output also appears in this occurrence.
Host stops use the reviewed TCG GDB protocol, with no guest memory/register
writes or injected delivery. Timing-sensitive success is separately retained
by the undisturbed run.

## Storage and capacity

Source ownership and actual native address/call/frame records retain four
local objects: KernelState, TaskStateStorage, the interrupt table and shared
context. The source’s nonreturning calls remain real suspended native calls;
borrowed pointer homes are not mistaken for their backing allocations.
Boundary 71 passes the existing two owned 16-byte roots to selected entry 1,
with exact provider-plan and argument-copy correspondence.

The image supplies 4,155,648 usable private-stack bytes, excluding saved state.
Each dedicated interrupt stack has 4,080 bytes below its aligned top. Composed
fatal-entry demand is 359 bytes and timer demand is 736 bytes, leaving margins
of 3,721 and 3,344 bytes. Current-stack entries require a 374-byte reserve,
added by the independently replayed physical-entry derivation.

Backing and lifetime rely on the adopted exact loader/receiving contract and
source/native retention review. The pinned EDK2 loader allocates and initializes
the full image, and successful ExitBootServices does not unload it. The kernel
restores its original translation root before installation and does not reuse
the private suffix. Actual reads corroborate the episode; they do not themselves
grant memory permission or prove every page-table entry or future delivery.

## Integrated compiler check

After resolving the actual main conflict, compiler source
`80a1badee90e1e3a24f1f622baec7c3c406e899d` completes the same saved native
request in 248.108 seconds, with unchanged inputs, no resource stop and all
33 entries retained. Its image bytes and artifact identity exactly equal R13.
This is a fresh source-free native completion; source checking and the two VM
observations keep their original identities. R17 result SHA256 is
`7ff2172e72e17466b031eb474fa0df884f20a4f087cf8eb2bf1a991a39a829ed`.

The exact current CLI also rejects the original borrowed-context use after
acknowledgement consumption and the incompatible registrar handler. The first
control reaches its intended live-borrow diagnostic in 7.442 seconds; the
second reaches its original handler-satisfaction diagnostic in 6.417 seconds.
The second recording check used a paraphrased sentence and remains failed;
separate readback verifies the complete current diagnostic bytes equal the
pinned historical diagnostic. No compiler run or original result was relabeled.

## Controls and retained evidence

All 33 original entry/calling-plan controls pass. Altered state/origin metadata,
missing bridges and invalid ordinals reject for each entry. Retained compiler
controls at `6c822102aed4` reject a missing explicit bootstrap environment
condition and acknowledgement of another installation’s controller. The
observer’s retained failure controls exercise
corrupted frames, state and guards. Observed XMM values are zero; this run does
not claim a nonzero SIMD-pattern test. Pending acknowledgement debt is not sampled
inside the handler; its completed state and repeated delivery are observed.
No fatal delivery is injected into this boot.

The first stopped attempt failed because two context-guard labels were omitted
from its plan. It remains failed; the retry adds only those labels. The R16
export wrapper also retains its integer-versus-formatted-MachineId comparison
failure. A separate strict saved-record readback qualifies its unchanged data;
no native run or original result was relabeled.

Raw logs, plans, inputs and independent reviews stay out of Git under
`build/native-main-integration/`, tied to the identities above. The main records
are `native-completion-r28-r13-original-parent/{uefi-undisturbed-r1,uefi-observed-r1}`,
`owner-homes-r28-r2/owner-backing-review.md`, the R16 saved readback, and
`full-feature-publication-review-r1/`. The historical claim
`599a808b70f94d9e8900a006d97f2cb6` records the compiler chain. This note records
bounded kernel boot evidence; it does not qualify the normal desktop, recovery
from fatal exceptions, multiple CPUs or physical hardware.
