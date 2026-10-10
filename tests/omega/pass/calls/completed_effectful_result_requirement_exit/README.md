The inner call increments a counter and publishes an integer normal-result
bound. The outer call proves its requirement from that completed result,
without replaying the effectful inner call. Native and Terminal execution must
return 0 after observing both the value 7 and exactly one counter increment.
The out-of-bound companion is `effectful_result_outside_captured_requirement`.
