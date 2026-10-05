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
diagnostic above.

## Why this is pending, not in `fail/`

The same harness refusal described in
`artifact_only_required_output_completion/README.md` reaches this case first:

    package-aware build filesystem access requires compiler-validated
    canonical Source metadata

Move it into `fail/` with `expected.txt` holding the sticky-failure fragment
above, and its golden row, once the harness can bind a fixture root with
canonical Source metadata.
