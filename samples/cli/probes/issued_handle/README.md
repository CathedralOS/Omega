# Issued handle

The program issues a handle carrying 41, reads that value through a borrow,
then consumes the handle once. The entry returns normally when it reads 41.

On Linux x86-64 or macOS ARM64, run
`mbx xtask sample-gate --filter samples/cli/probes/issued_handle`.

The issuer proves its result qualification at normal completion; the annotation
does not authorize other machines to manufacture equal handles. Borrowing retains
ownership, while the consuming call removes the caller's cleanup obligation.
This exercises the smaller prerequisite for the Windows filesystem handle contract.
