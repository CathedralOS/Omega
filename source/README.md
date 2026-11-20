# Omega in Omega

The Omega implementation keeps its hosted applications, compiler packages and
shared foundations in the same roles as the [Rust implementation](../source-rust).
It also ships the language libraries that Omega programs import.

| Directory | Responsibility |
| --- | --- |
| [application/ocreq](application/ocreq) | Read a sealed compiler request from stdin and publish its outcome on stdout. |
| [application/construction](application/construction) | Read a closed set of source files and ask the compiler to emit an Alpha tape. |
| [subsystems/compiler/request](subsystems/compiler/request) | Decode request bytes and construct outcomes independently of console I/O. |
| [subsystems/compiler/driver](subsystems/compiler/driver) | Run the frontend and hand its lowered program to the Alpha backend. |
| [subsystems/compiler/source_to_psi](subsystems/compiler/source_to_psi) | Own the source representations and the stages that parse, resolve, check and lower a program. |
| [subsystems/compiler/alpha](subsystems/compiler/alpha) | Encode the supported scalar operations as Alpha tape. |
| [foundation/wire](foundation/wire) | Decode little-endian words for both compiler inputs. |
| [library](library) | Expose core, allocation, standard-library and blocking-executor code to Omega programs. |

A well-framed public request currently reaches the unimplemented request
semantics phase and produces an Incomplete outcome. The construction application
can exercise more of the compiler by taking package ownership, dependencies
and an entry point as inputs. Compiling the compiler's own sources into a
runnable tape remains open. [Construction input and limits](application/construction/README.md)
