# Pass 33 callbacks through one call

A program can pass 33 callback functions through one call. This test keeps them
in a plan for one borrowed table. The receiving Omega function, called the
registrar, executes the first, second and last selected callbacks with distinct
results and checks adjacent and outer fields. Native and Terminal execution
must both exit 0.

The calling policy matches each table slot to the callback contract it expects,
including a distinct second contract. The fixture retains its ARM64 calling
plan. Its receiving function calls selected machines directly; it does not load
the stored native addresses. The companion
`layouts/private_callback_descriptor_fragments_exit` fixture checks that those
addresses are actually written and executes all 33 through a C function.
