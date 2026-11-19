# Access policies and operations

The [Psi access records](access.rs) describe operations, observation models, field permissions, service reach and the capabilities a resource offers. Typed trees, Psi encoding and the interpreter use these same descriptions.

The [interpreter](../../../interpreter/memory/storage-access/) checks a live operation against its grant, borrow and source loan. The language specifies how a checked placement and resource profile establish the operands for that decision. [Access policies and views](../../../../../wiki/spec/resources/access.md)
