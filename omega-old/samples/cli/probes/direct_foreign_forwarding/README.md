# Direct foreign forwarding

On Windows, `omega run samples/cli/systems/direct_foreign_forwarding/main.omg`
must exit 0. A direct foreign call returns the real process identity, while the
selected inline implementation calls that same leaf and returns 23.

The baseline refuses the exact `NativeProcess::identity` dependency even though
its foreign binding and requirement contract are checked. The leaf and wrapper
remain distinct callables; selecting the wrapper must not replace the leaf.
