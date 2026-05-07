# Stdin Line Buffering Runtime Canary

This canary captures the runtime behavior we need before console samples are
honest under both terminal and piped input.

The desired behavior is:

- one `console.read_line(mut line)` consumes exactly one logical line
- trailing newline is not part of `line.text`
- unread bytes after the newline remain available for the next read
- EOF after the provided lines terminates cleanly instead of spinning

This is not wired into the automated test runner yet.
