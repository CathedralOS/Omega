# Borrowed stores through generic owners

A checked call through a nested generic owner preserves the exact caller destination and input backing of a stored borrowed descriptor.

The stored view must change from [11,29] to [31,47], and both caller arrays remain readable. The match alternate has distinct storage, so selecting the wrong destination traps. Constant execution observes the view before returning a Boolean, never a loan. Reject twins attempt to store a view of callee-local storage.

The selected-target boundary declares the input region carried by its output descriptor. Provider bodies still perform the same checked nested generic store; the contract does not authorize callee-local backing.
