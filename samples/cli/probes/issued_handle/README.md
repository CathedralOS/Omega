# Issued handle

Run `omega run samples/cli/systems/issued_handle/main.omg` on macOS ARM64.
Expected exit: 0. The program issues an affine handle carrying an authorized
domain, reads its value through a borrow, then consumes the handle once.

The issuer proves its result qualification at normal completion; the annotation
does not authorize other machines to manufacture equal handles. Borrowing retains
ownership, while the consuming call removes the caller's cleanup obligation.
This exercises the smaller prerequisite for the Windows filesystem handle contract.
