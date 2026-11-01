TYPED-FROZEN-BEFORE-CHECKING: complete inferred invocation regions before freezing typed trees.
The statement call has a Unit result and an unused conformance binder. Its required
region and both caller regions have independently declared names. Both calls
retain their exact complete tuple. The twin changes only the first selected
conformance actual from the first region to the second; erased carriers agree,
but exact conformance admission must reject.
