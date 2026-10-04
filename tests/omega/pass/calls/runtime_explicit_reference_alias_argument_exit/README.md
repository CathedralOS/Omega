A derived field loan is passed to direct and forwarding calls. Both writes must
reach the original owner while its disjoint neighbor remains 85. The nested
loans end at their last use before the caller observes the fields; ending them
does not discard their captured backing from the earlier call plans.

Both engines must exit 0. From the repository root, prepare the project's
ordinary package review once:

```text
omega update --project tests/omega/pass/calls/runtime_explicit_reference_alias_argument_exit
```

The fixture requires no capability decisions on a fresh macOS ARM64 project,
so update publishes the lock directly. If a changed package graph requires a
review, edit the reported document's individual decisions and complete it with
`omega update --resume --project tests/omega/pass/calls/runtime_explicit_reference_alias_argument_exit`.
Then run:

```text
omega run --both tests/omega/pass/calls/runtime_explicit_reference_alias_argument_exit/main.omg
```

Comparison reads stdin to EOF. This fixture needs no input: use `</dev/null` on
macOS/Linux or redirect from `NUL` under Windows `cmd`.

The isolated regression uses the corpus runner's test package policy:

```text
python tools/corpus_gate.py --both --filter calls/runtime_explicit_reference_alias_argument_exit
```
