# Default operand region custody

`omega --check main.omg` must succeed. The unused invocation binder belongs to
`Reader::read`; `Value` captures `Mapped`'s public region. Default substitution
must retain those two supplying owners rather than reinterpret both under the
synthesized row's invocation telescope.

The baseline compiler at `75a8abc59f7` and the current repair candidate both
refuse this intended-valid source. It is retained pressure for
`PSI-TARGET-FAMILIES`, not an expected-rejection control.
