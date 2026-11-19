# residual_cleanup

First sample with mixed linear+affine owners (`Bundle {
receipt, note }`): moving the linear receipt out leaves the note
as a hooked residual; both residuals dispose in reverse
production order at exit.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
