# Assembly imports in Build and the program

Build and the program both import the core assembly declarations. The program
must still be allowed to call helpers that declare the hardware services used
by their instructions. Build's separate declarations must not replace those
belonging to the program.

This fixture checks both `MachineControl` and `PortIo`, including propagation
through ordinary helper calls. It is checking-only: executing `hlt` or a port
write requires an appropriate privileged environment. The existing
`target_family_instruction_exit` fixture supplies hosted native/Terminal
coverage of safe assembly instructions.

The negative `asm_service_requires_core_owner` fixture defines ordinary traits
with the same names. Those declarations cannot substitute for core assembly.
