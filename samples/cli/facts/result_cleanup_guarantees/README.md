# Normal result cleanup guarantees

The shared Seed.ready == true observation survives each place into make_token(&seed), whose completed
result is ready for normal shared-call cleanup.

The root retains the original once-only effectful producer reproduction. The
nine child projects carry the same copied Seed readiness through local, field,
parameter, call result, match join, const name, another state, another machine,
and target body. Each invalid child changes only that carried readiness to
false; the producer relation and cleanup prerequisite stay the same. The producer
borrows the Seed so its normal postcondition observes caller storage. A const
stores the copyable Seed, and every operation constructs a fresh affine Token.
The target-body place performs the operation inside each target body. This row
does not establish owned-input post-state transport or common Boolean result
facts imported from a portable target-family head.

Run each project with `omega run --both <project>/main.omg`. The authored exit
is 0 in native and Terminal execution. Run each invalid child with `omega
--check <project>/invalid/main.omg`; it must refuse normal cleanup.
