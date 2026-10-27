# PR348 nested-machine migration evidence

`observations/integrated-acceptance.json` states the measured acceptance.
`manifest.json` binds source, producers, qualification, and the golden-only
publication delta. `qualification/` retains the immutable capture and validator.
`source/` bundles restore the actual unlanded source revisions using their
published main prerequisite; `producer-builds/` retains optimized build records.

The native/Terminal observations are macOS ARM64. All nine places and their nine
fact-sensitive twins pass; the focused corpus retains 169 source fixtures and
80 actual executions (79 exit zero, one authored trap). The 100000-cycle native
witness also passed under a 256 KiB OS stack limit. This is not a full portable
stack proof or other-host release qualification.

Raw initial observations remain visible. Two former positive tests demanded
non-tail runtime recursion, contrary to the accepted termination contract. They
were moved to negative controls and freshly rejected for their non-tail edges.
`historical/` retains the original Nat/private-premise failures, the wrong public
acceptance, and the ten parser failures reproduced on the integration's main base.
No historical execution is labeled as a run of the integrated producer.
