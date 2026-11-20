# Schema descriptions

A [schema](schema.rs) retains the declaration, fields, cases and nominal references visible to its requester. Its [nodes](nodes.rs), [revision](revision.rs) and [runtime image](runtime_image.rs) describe the same checked graph. Decoding an image validates its shape and primitive identities.

The [source type checker](../../source-to-psi/representations/typed-trees/src/declaration_reflection.rs) settles queries using the requester's visibility. Psi and runtime consumers read the resulting description. [Reflection and runtime visitation](../../../../../wiki/spec/language/reflection.md)
