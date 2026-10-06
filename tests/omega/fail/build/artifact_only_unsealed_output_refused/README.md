# An unsealed staged file cannot complete a required output

The sealing control for `artifact_only_required_output_completion`. This build
omits only the `BuildOutput::close` that retires the writer, so `manifest.txt`
still has a live writer when the build asks to complete its obligation.

`complete` cannot accept an open writer, so it answers
`OutputCompletion::Retry`, handing the obligation and the file custody back. The
obligation is linear, so the retry arm must dispose of it; this build's
disposition is the authored `fail`, and that failure is sticky, so the
activation can never publish a successful product set:

    required output `manifest.txt` of `build` failed: staged output never sealed

Without this control, a regression that made `complete` answer `Sealed`
unconditionally would leave the completion case green, because that case
observes the seal only through its own success.

Verified through the real CLI on exactly this source, which produced the
diagnostic above. Its `expected_published.txt` is empty: the corpus builds and
publishes the fixture as the CLI does and records `published:none`.

## How the corpus hosts it

This case reached the harness's own refusal before the rule it names, because
the runner bound the root package straight to the writable fixture directory
and so had no canonical Source metadata to offer a package-aware build
activation. The runner now stages every fixture root as a sealed private
snapshot and binds the root package to it, as
`pass/build/artifact_only_required_output_completion/README.md` describes, so
the rejection recorded here is the authored one.
