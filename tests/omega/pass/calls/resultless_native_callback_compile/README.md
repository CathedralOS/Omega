# Resultless native callback

Compile a synchronous `void(void)` initializer through a real Linux x64
`pthread_once` import, with its mutable once word supplied as an ordinary
borrowed argument. The selected callback remains resultless; no dummy integer
return or invented export substitutes for it.

`libc.so.6` with `pthread_once@GLIBC_2.34` requires glibc 2.34 or newer.
The [POSIX declaration](https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/pthread.h.html)
specifies the callback signature. The retained import version and once-word
type follow glibc's [implementation](https://github.com/bminor/glibc/blob/master/nptl/pthread_once.c)
and [type declaration](https://github.com/bminor/glibc/blob/master/sysdeps/nptl/bits/pthreadtypes.h).

This is compile coverage, not an invocation oracle or Windows callback
acceptance. Run its artifact gate with
`python3 tools/corpus_gate.py --native-target linux_x86_64 --filter calls/resultless_native_callback_compile`.
