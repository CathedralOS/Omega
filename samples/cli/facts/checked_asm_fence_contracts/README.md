# Checked fence contracts

Owning task: **CHECKED-ASM-LABEL-CONTRACTS**, catalog and transitive plans.

Checked fence sequences retain their catalog target, ordering and exact empty clobber set while carrying input 37 to a result requiring 37.

The x86 bodies execute LFENCE, SFENCE, MFENCE in that order. The AArch64
bodies execute full-system DMB LD, ST, SY in that order. These retain distinct
catalog identities and access masks; DMB LD also orders later stores and is
not equated with LFENCE. All four hosted targets bind `ProgramEntry`.

The root and nine places call a consumer requiring exactly 37, before their
runtime equality oracle. Each `invalid/` changes only the declared clobbers:
rax on x86, x0 on AArch64. Both are invented changes for this catalog sequence,
so every twin must reject for the exact clobber-union obligation. The sequence,
bound, scalar guarantees and consuming operation are otherwise byte-identical.
Pins are `tests/omega/{pass,fail}/facts/checked_asm_fence_contracts_through_*`.

- Establishment: the caller proves input == 37; entry and exit assertions are
  goals. Each catalog operation preserves that exact value and changes no
  general register or flag. Its ordering event remains a separate obligation.
- Identity, lifetime and composition: each place carries the same scalar
  subject through ordinary fields, parameters, returns, joins, constants,
  nested machines, non-nested machines and selected target bodies. Helpers
  retain the ordering events; a join executes only its selected branch.
- Terminal realization: exact target and fence kind cross the canonical wire.
  Execution requires the receiving host's explicit private sequential memory
  model, records each completed fence in order, and retains progress across
  fuel pauses. Missing supply and mismatched targets must reject before effects.
  This model supplies neither shared/device memory nor synchronization proofs.
- Native realization: each selected kind has an independent byte decoder and
  a catalog-ordering footprint. The final span must retain the exact form,
  unchanged stack, empty clobbers and external-effect barrier. Tests must reject
  missing, doubled, truncated, substituted and wrong-target fence bytes.

Baseline f5d7c12b93bbd27d09cfa4ba8d129a7bbbc63ad2 accepted the known x86 source
but refused `omega run --both` with no executable body at the fence call. That
source/oracle and optimized producer are retained in the claim qualification.
Current acceptance is pending; no completion is inferred from file presence.

The parent remains open for the other catalog families, hardware-effect linear
retirement, consumer-owned PortIo settlement, supplied device/interrupt/idle
models, external entry and customer canaries. Hosted execution observes Linux
x86_64; foreign target interpretation and emitted images do not establish foreign
native execution, hardware timing, device completion or multi-core ordering.
