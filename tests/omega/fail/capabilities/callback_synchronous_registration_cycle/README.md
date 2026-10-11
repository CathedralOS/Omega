# Same-stack callback registration cycle

This is the future-only registration witness with a conservative extra
`invokes Initializer;` promise on the registrar. The destructor invokes the
registrar, closing a synchronous cycle. Provider selection must reject that
cycle even though the concrete pthread_key_create implementation never invokes
its destructor during registration. Neither emitted addresses nor a successful
future-only code link may discharge this contract.
