# Tile

A square-cornered block that holds one idea, drawn in outline for target-neutral things and solid for realized ones.

- `outline` (the default, on `surface`) is for source, checking, proofs and Terminal Psi: things that hold for every target.
- `solid` (in `ink`) is for native code, executables and target-specific results.

You provide an optional `label` (mono, for a stage or path), an optional `title`, and the body as children. The container sets the width; put tiles in a grid with `gap`.

Do let the variant say which side of the Psi and Omega boundary the content sits on. Don't nest tiles, don't round their corners and don't add shadows.
