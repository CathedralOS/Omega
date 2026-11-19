# Native stage replay

An explicit validator request keeps the original stage outputs and checks the
published object against them. The [object verifier](published-objects/src/lib.rs)
returns diagnostics if the requested originals are missing or belong to another
body. It checks root, callable and component bodies using their own retained
inputs. Its output is a verdict on an existing object.

The producer can retain the issued allocation, physical machine, encoding and
baseline layout in `RetainedPhysicalInputs`. The verifier reads those original
records to replay selected rewrites, allocation, frame construction and layout.
Frame and encoding checks live beside the [machine-code representation](../pipeline/representations/machine-code/).
The numbered stages own instruction selection, encoding and object emission.
