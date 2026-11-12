# Reject a foreign callback without its entry plan

The registrar is an actual Linux libc `atexit` binding, while the supplied
handler deliberately has no selected callback calling plan. The control must
reach that missing-plan refusal, rather than fail because no registrar provider
was selected. The locator is authored in Build under the current binding
contract; the retired source-level `via` forwarder supplies no binding.

The Product call and handler remain unchanged. Its diagnostic oracle requires
the explicit selected callback calling plan error; a missing provider does not
satisfy it. The invalid program must not execute the foreign call.
