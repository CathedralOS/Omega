# Checked operator contracts

The original operator/provider pair retains its checked requires and ensures,
including renamed parameters. The second pair requires a Wrapping cast from
`u16` to `u8`; its provider must preserve that exact arithmetic policy.

The entry calls both operators and verifies the original result 70 and the
Wrapping result 0 for input 256. A Saturating implementation would return 255
and trap. Build selects the checked provider and entry on all four hosted targets.
Each entry promises native/Psi exit 0; actual qualification remains pending.

The existing negative fixture retains its original missing-contract provider
and adds a provider that substitutes Saturating for the required Wrapping cast.
The original message pin remains unchanged. Required source locations must
attribute both refusals to their distinct provider declarations.
