This is the full-input-contract control for NESTED-MACHINE-UNIFICATION's
[private entry inference](../../../../../wiki/spec/language/state_contracts.md#private-entry-inference).
The signed guard excludes zero and the primitive `MIN / -1` pair; the unsigned
guard transports a saved Boolean relationship through a private helper.
The outer `requires true` contracts prevent either proof from specializing
against `main`'s convenient arguments. Every arrival must prove the private
helper's inferred requirement. Native and Terminal Psi exits must both be zero;
the runtime oracle exercises ordinary division, zero, and the signed excluded
pair. The existing `private_division_signed_pair_control`,
`private_division_mixed_arrival_control`, and
`private_division_changed_storage_control` cover missing guards and changed
subjects. Broader local-summary, custody, and recursive acceptance remains open.
