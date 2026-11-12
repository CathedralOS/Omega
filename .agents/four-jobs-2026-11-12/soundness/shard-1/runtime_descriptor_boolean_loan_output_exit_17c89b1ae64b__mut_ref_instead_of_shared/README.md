# Exact Boolean backing at a descriptor boundary

On macOS AArch64, `omega run --both main.omg` exits zero and writes exactly
`01 0A`. The first byte is the canonical Boolean encoding, not the text `true`.
An ordinary helper changes the original local; a shared loan carries that
backing through another helper and state before the foreign write. The newline
is a separate primitive-local loan, not part of the Boolean's one-byte extent.
