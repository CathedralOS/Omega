The false guard forwards completion to another top-level machine whose only
route is an authored crash. A reference-returning machine owes returned
reference origins only on routes that can complete normally, whichever machine
declares the destination; the nested sibling
`runtime_guarded_reference_return_crash` covers the same edge into a nested
machine. Native execution and every per-target Terminal installation must trap.
