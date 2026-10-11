# Default operand region custody

`omega --check main.omg` must succeed. The unused invocation binder belongs to
`Reader::read`; `Value` captures `Mapped`'s public region. Default substitution
must retain those two supplying owners rather than reinterpret both under the
synthesized row's invocation telescope.

The supplying conformance declaration stays attached to the copied operand
through syntax, resolution and typing. Fresh signature nodes keep their member
telescope; no spelling or source-span match assigns ownership.
