# Reject a foreign callback without its entry plan

The registrar is an actual Linux libc `atexit` binding, while the supplied
handler deliberately has no selected callback calling plan. The control must
reach that missing-plan refusal, rather than fail because no registrar provider
was selected. The locator is authored in Build under the current binding
contract; the retired source-level `via` forwarder supplies no binding.

Build selects the declared `Registrar::install` member family. The Product
call remains `Registrar<u64>::install<CallbackBody::notify>()`, so the closed
trait application stays part of the witness. A generic-operand or provider
selection error does not satisfy its unchanged diagnostic oracle.
