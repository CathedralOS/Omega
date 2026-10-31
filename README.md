# PR1023 machine-header delimiter evidence

Submitted source: `5e183ca227da696ac4549f5c38675158711dffe4`; fixed base: `2feafe36a0b7de29a61055e003996dd7574ea8c1`.

The manifest binds original observations, producer receipts and independent review. It does not turn reproduced baseline failures, unexecuted hosts or broader REQUIRES/NESTED acceptance into passes. Native producer binaries remain immutable in the original workbench.

The original C6 corpus stamp source is retained in the first Git bundle. In a clone containing the fixed published base prerequisite, run `git bundle unbundle source-2fc408782574caca80f6fa48deee61c767f1616e.bundle`, then unbundle the final-source bundle, whose prerequisite is C6. Both bind their original exact commits; source subjects are not relabeled as final-head executions.

The lower-chain raw stdout retains an obsolete16-invocation success label. The source contains20 calls; final changes only that label after all checks. Actual expected bytes were000000000041, receipt148.663s, D customer6529.480s, subprocess and original wrapper exit0, empty stderr.
