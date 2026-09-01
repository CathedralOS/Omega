# `source/alpha/` — the native tape executor

Alpha is the execution floor of the compiler lattice. It is a small,
hand-auditable virtual machine with byte input, byte output, and a fixed binary
instruction encoding. Alpha has no textual programming language and no
assembler grammar.

The first portable program is the Beta assembler tape owned by
[`../beta/compiler/`](../beta/compiler/). That tape reads Beta assembly text and
emits Alpha tape bytes. Keeping that program outside Alpha makes the boundary
literal:

```text
Alpha: audited native VM + raw tape encoding
Beta:  textual assembly + direct Alpha implementation
```

## Owned files

```text
alpha_x64_windows.exe    audited Windows x64 VM container
alpha_x64_windows.hex    annotated x64 audit listing
alpha_arm64_macos        audited macOS arm64 VM container
alpha_arm64_macos.s      hand-authored arm64 implementation
alpha_arm64_macos.lst    committed arm64 disassembly
SEMANTICS.md             AlphaBootstrapV2 execution and tape semantics
seed_env.sh              host seed selection and tape stamping
conformance.sh           executable opcode and boundary checks
verify.sh                local seed, semantics, and Beta-tape reconstruction gate
checker/                 rooted derivation-checker service beside the lattice
alpha_ref.py             temporary untrusted executable reference
```

`verify.sh` checks the selected host VM in three parts:

- provenance where a reproducible native forge is available;
- behavior against every opcode edge in `SEMANTICS.md`;
- reconstruction of Beta's committed assembler tape from
  `../beta/compiler/assembler.beta`.

`verify.sh --edge` omits the native-container provenance diagnostic. The direct
compiler lattice starts with the selected audited Alpha VM; rebuilding that VM
from host assembly is useful supply-chain evidence, not another language rung.

## Tape identity

Both committed VMs implement `AlphaBootstrapV2`: 256 MiB of semantic memory, a
hidden return stack, and an exact one-MiB embedded-tape extent including its
four-byte length. The maximum raw tape is therefore 1,048,572 bytes.

The native containers are platform-specific. Alpha tapes are not. A compiler or
checker artifact is identified by its raw tape bytes, never by a stamped PE or
Mach-O container. `stamp_seed` is loading plumbing: it copies `[length | tape]`
into the selected VM container and, on macOS, restores the OS-required code
signature. It does not compile or change the tape's meaning.

The Windows and macOS VMs are independent native realizations of the same Alpha
semantics. `alpha_ref.py` is a third, untrusted diagnostic implementation. The
written semantics and audited native correspondence carry authority;
cross-implementation agreement is regression evidence only.

## Retention inventory

| Retained child/files | Direct role | Deletion condition |
| --- | --- | --- |
| `checker/` | Rooted certificate-checker service used beside compiler edges. | Delete when an equally low or lower accepted checker replaces it. |
| `SEMANTICS.md` | Authoritative Alpha execution and raw-tape relation. | Replace only atomically with a ruled Alpha revision and every consumer. |
| `alpha_arm64_macos`, `alpha_arm64_macos.s`, `alpha_arm64_macos.lst` | Audited macOS arm64 realization, source, and listing. | Delete only with platform retirement or an equally audited replacement. |
| `alpha_x64_windows.exe`, `alpha_x64_windows.hex` | Audited Windows x64 realization and listing. | Delete only with platform retirement or an equally audited replacement. |
| `seed_env.sh` | Select and stamp the host VM without changing tape identity. | Delete when every caller can execute raw tapes through an equally audited interface. |
| `conformance.sh`, `verify.sh` | Pin execution semantics and the direct Beta-program edge. | Delete only when a stronger checked admission subsumes the same facts. |
| `alpha_ref.py`, `diamond-py.sh` | Temporary independent semantics comparison. | Delete together when the checked realization relation subsumes the diagnostic. |

The Beta language, assembler source, direct assembler tape, examples, and
self-host gate live in [`../beta/`](../beta/). There are intentionally no
`.alpha` source files: Alpha programs are tapes.
