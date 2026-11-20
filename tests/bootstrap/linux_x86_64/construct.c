/* Build a Gamma compiler with its predecessors. Only those
   programs read source syntax. This driver moves bytes and checks exits. */
#include <asm/stat.h>
#include <asm/unistd.h>
#include <linux/errno.h>
#include <linux/fcntl.h>
#include <linux/stat.h>
#include <stddef.h>
#include <stdint.h>

extern long kernel(long number, ...);
extern const unsigned char alpha_prefix[], alpha_suffix[], beta_root[], beta_end[];

static int repository;
static int target_alpha;
static const char *operation;
static const char partial[] = "stage.partial";
static unsigned char chunk[16384];
enum { hole_size = 0x1000000 };

static void message(const char *s) {
    size_t n = 0;
    while (s[n]) ++n;
    kernel(__NR_write, 2, s, n);
}

static _Noreturn void fail(const char *reason) {
    message("bootstrap: ");
    message(operation);
    message(": ");
    message(reason);
    message("\n");
    kernel(__NR_exit, 1);
    __builtin_unreachable();
}

static long checked(long result, const char *reason) {
    if (result < 0) fail(reason);
    return result;
}

static int open_input(int dir, const char *path) {
    return checked(kernel(__NR_openat, dir, path, O_RDONLY | O_CLOEXEC), path);
}

static int create(const char *path) {
    return checked(kernel(__NR_openat, AT_FDCWD, path,
                          O_CREAT | O_EXCL | O_WRONLY | O_CLOEXEC, 0600), path);
}

static void close_file(int fd) {
    checked(kernel(__NR_close, fd), "close failed");
}

static long read_some(int fd, unsigned char *bytes, size_t count) {
    long n;
    do {
        n = kernel(__NR_read, fd, bytes, count);
    } while (n == -EINTR);
    return checked(n, "read failed");
}

static void write_all(int fd, const void *data, size_t count) {
    const unsigned char *bytes = data;
    while (count) {
        long n = kernel(__NR_write, fd, bytes, count);
        if (n == -EINTR) continue;
        if (n <= 0) fail("write failed");
        bytes += n;
        count -= n;
    }
}

static void copy(int src, int dst) {
    long n;
    while ((n = read_some(src, chunk, sizeof chunk)) != 0)
        write_all(dst, chunk, n);
}

static void copy_file(int dir, const char *path, int dst) {
    int fd = open_input(dir, path);
    copy(fd, dst);
    close_file(fd);
}

static long seek(int fd, long offset, int origin) {
    return checked(kernel(__NR_lseek, fd, offset, origin), "seek failed");
}

static void finish(int fd, const char *name, int executable) {
    if (executable)
        checked(kernel(__NR_fchmod, fd, 0700), "chmod failed");
    close_file(fd);
    checked(kernel(__NR_rename, partial, name), "rename failed");
}

/* Preserve the audited Linux executable. The hole contains a little-endian
   length, the exact Alpha program, then zeroes up to the original trailer. */
static void stamp(const char *tape, const char *image) {
    operation = image;
    int src = open_input(AT_FDCWD, tape);
    long size = seek(src, 0, 2);
    if (size > hole_size - 4) fail("tape exceeds the seed hole");
    seek(src, 0, 0);
    int dst = create(partial);
    size_t prefix_size = (uintptr_t)alpha_suffix - (uintptr_t)alpha_prefix;
    uint32_t length = size;
    write_all(dst, alpha_prefix, prefix_size);
    write_all(dst, &length, sizeof length);
    copy(src, dst);
    close_file(src);
    seek(dst, prefix_size + hole_size, 0);
    write_all(dst, alpha_suffix, (uintptr_t)beta_root - (uintptr_t)alpha_suffix);
    finish(dst, image, 1);
}

/* Alpha's byte writer cannot report a host disk error. Drain a blocking
   pipe in the parent, checking every file write before publishing a stage. */
static void run(const char *program, int input, const char *output, int executable) {
    int ends[2], status;
    operation = output;
    int dst = create(partial);
    checked(kernel(__NR_pipe2, ends, O_CLOEXEC), "pipe failed");
    long child = checked(kernel(__NR_fork), "fork failed");
    if (child == 0) {
        checked(kernel(__NR_dup2, input, 0), "stdin failed");
        if (input == 0)
            checked(kernel(__NR_fcntl, 0, F_SETFD, 0), "stdin unavailable");
        checked(kernel(__NR_dup2, ends[1], 1), "stdout failed");
        const char *argv[] = {program, NULL};
        const char *env[] = {NULL};
        kernel(__NR_execve, program, argv, env);
        fail("cannot execute stage");
    }
    close_file(input);
    close_file(ends[1]);
    copy(ends[0], dst);
    close_file(ends[0]);
    long waited;
    do {
        waited = kernel(__NR_wait4, child, &status, 0, NULL);
    } while (waited == -EINTR);
    checked(waited, "wait failed");
    if (status != 0) {
        message("bootstrap: ");
        message(output);
        if (status & 127) {
            message(": producer terminated by signal\n");
            kernel(__NR_exit, 128 + (status & 127));
        } else {
            message(": producer exited unsuccessfully\n");
            kernel(__NR_exit, (unsigned)status >> 8);
        }
        __builtin_unreachable();
    }
    if (seek(dst, 0, 1) == 0) fail("producer returned an empty artifact");
    finish(dst, output, executable);
}

static void require_equal(const char *left, const char *right) {
    unsigned char other[sizeof chunk];
    int a = open_input(AT_FDCWD, left), b = open_input(AT_FDCWD, right);
    long count;
    while ((count = read_some(a, chunk, sizeof chunk)) != 0) {
        long received = 0;
        while (received < count) {
            long n = read_some(b, other + received, count - received);
            if (!n) fail("rebuild differs in length");
            received += n;
        }
        for (long i = 0; i < count; ++i)
            if (other[i] != chunk[i]) fail("rebuild differs");
    }
    if (read_some(b, other, 1)) fail("rebuild differs in length");
    close_file(a);
    close_file(b);
}

/* The evaluator takes a length followed by the Gamma program and then
   its input. Here the source compiler compiles its own unchanged source. */
static void prepare_source(void) {
    operation = "Gamma compiler input";
    int fd = create("compiler.gamma");
    copy_file(repository, "bootstrap/gamma/compiler/compiler.gamma", fd);
    write_all(fd, "\n", 1);
    copy_file(repository, "bootstrap/gamma/compiler/frontend.gamma", fd);
    write_all(fd, "\n", 1);
    copy_file(repository, target_alpha ? "bootstrap/gamma/compiler/alpha/alpha_backend.gamma"
                                       : "bootstrap/gamma/compiler/linux_x86_64/native.gamma", fd);
    close_file(fd);

    int src = open_input(AT_FDCWD, "compiler.gamma");
    long size = seek(src, 0, 2);
    if ((unsigned long)size > UINT32_MAX) fail("source exceeds Gamma length field");
    uint32_t length = size;
    fd = create("compiler.request");
    write_all(fd, &length, sizeof length);
    seek(src, 0, 0);
    copy(src, fd);
    seek(src, 0, 0);
    copy(src, fd);
    close_file(src);
    close_file(fd);
}

static void snapshot_input(int input, const char *name) {
    int output = create(name);
    copy(input, output);
    close_file(input);
    close_file(output);
}

/* The native backend emits an executable image. The Alpha backend emits a
   tape, which runs only after the same audited seed has been stamped. */
static void compile_stage(const char *producer, const char *input,
                          const char *image, const char *tape) {
    int source = open_input(AT_FDCWD, input);
    run(producer, source, target_alpha ? tape : image, !target_alpha);
    if (target_alpha) stamp(tape, image);
}

static int compare(const char *a, const char *b) {
    while (*a && *a == *b) { ++a; ++b; }
    return (unsigned char)*a - (unsigned char)*b;
}

static int regular_file(int dir, const char *path) {
    int fd = checked(kernel(__NR_openat, dir, path,
                            O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK), path);
    struct stat stat;
    checked(kernel(__NR_fstat, fd, &stat), path);
    if (!S_ISREG(stat.st_mode)) fail(path);
    return fd;
}

/* Open each member relative to the manifest directory, without traversing
   symlinks. Restore its spelling so duplicate checks can use the same text. */
static int member_file(int root, char *path) {
    int parent = root;
    char *part = path;
    for (;;) {
        char *end = part;
        while (*end && *end != '/') {
            if (*end < 33 || *end > 126 || *end == ':' || *end == '\\')
                fail("invalid member path");
            ++end;
        }
        int last = !*end;
        *end = 0;
        if (!*part || !compare(part, ".") || !compare(part, ".."))
            fail("invalid member path");
        if (last && (end - part <= 6 || compare(end - 6, ".gamma")))
            fail("invalid member suffix");
        int fd = last ? regular_file(parent, part) :
            checked(kernel(__NR_openat, parent, part, O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC), part);
        if (parent != root) close_file(parent);
        if (last) return fd;
        *end = '/';
        part = end + 1;
        parent = fd;
    }
}

static int source_byte(unsigned char c) {
    return c == 9 || c == 10 || c == 13 || (c >= 32 && c <= 126);
}

static void append_source(int fd, int out, uint64_t expected, int check_length) {
    long n;
    while ((n = read_some(fd, chunk, sizeof chunk)) != 0) {
        if (check_length) {
            if ((uint64_t)n > expected) fail("member length mismatch");
            expected -= n;
        }
        for (long i = 0; i < n; ++i)
            if (!source_byte(chunk[i])) fail("invalid Gamma source byte");
        write_all(out, chunk, n);
    }
    if (check_length && expected) fail("member length mismatch");
    close_file(fd);
}

static char *line(char **cursor) {
    char *start = *cursor, *end = start;
    while (*end && *end != '\n' && *end != '\r') ++end;
    if (*end) {
        int cr = *end == '\r';
        *end++ = 0;
        if (cr && *end == '\n') ++end;
    }
    *cursor = end;
    return start;
}

static char *field(char **cursor) {
    char *start = *cursor, *end = start;
    while (*end && *end != ' ') ++end;
    if (*end != ' ' || end == start) fail("invalid member row");
    *end = 0;
    *cursor = end + 1;
    return start;
}

static void check_hex(const char *s) {
    size_t i = 0;
    for (; (s[i] >= '0' && s[i] <= '9') || (s[i] >= 'a' && s[i] <= 'f'); ++i) {}
    if (s[i] || i != 64) fail("invalid member identity or digest");
}

static uint64_t decimal(const char *s) {
    if (!*s || (*s == '0' && s[1])) fail("invalid member length");
    uint64_t n = 0;
    while (*s) {
        unsigned digit = *s++ - '0';
        if (digit > 9 || n > (UINT64_MAX - digit) / 10) fail("invalid member length");
        n = n * 10 + digit;
    }
    return n;
}

static void pack_delta(void) {
    operation = "Delta compiler source";
    int dir = checked(kernel(__NR_openat, repository, "bootstrap/delta/compiler",
                             O_DIRECTORY | O_CLOEXEC), "Delta compiler directory");
    int members = checked(kernel(__NR_openat, dir, "implementation",
                                 O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC), "implementation directory");
    int manifest = regular_file(members, "implementation.gamma.sources");
    size_t size = seek(manifest, 0, 2);
    seek(manifest, 0, 0);

    /* A valid row has at least 64 bytes of identity. Keep the input and one
       pointer per possible row in a single allocation until exit. */
    size_t slots = size / 64 + 1;
    uintptr_t base = kernel(__NR_brk, 0);
    uintptr_t end = base + slots * sizeof(char *) + size + 1;
    if (end <= base || (uintptr_t)kernel(__NR_brk, end) != end) fail("manifest too large");
    char **paths = (char **)base;
    char *text = (char *)(paths + slots);
    for (size_t got = 0; got < size;) {
        long n = read_some(manifest, (unsigned char *)text + got, size - got);
        if (!n) fail("truncated manifest");
        got += n;
    }
    if (read_some(manifest, chunk, 1)) fail("manifest grew");
    close_file(manifest);
    for (size_t i = 0; i < size; ++i)
        if (!source_byte(text[i])) fail("invalid manifest byte");
    text[size] = 0;
    if (compare(line(&text), "GammaSourceClosureV1") || !*text) fail("invalid Gamma manifest");

    int output = create(partial);
    append_source(regular_file(dir, "delta_compiler.gamma"), output, 0, 0);
    size_t count = 0;
    const char *previous = "";
    while (*text) {
        char *row = line(&text);
        if (compare(field(&row), "member")) fail("invalid member row");
        char *identity = field(&row);
        check_hex(identity);
        if (compare(previous, identity) >= 0) fail("member identities out of order");
        previous = identity;
        uint64_t length = decimal(field(&row));
        check_hex(field(&row));
        for (size_t i = 0; i < count; ++i)
            if (!compare(paths[i], row)) fail("duplicate member path");
        paths[count++] = row;
        append_source(member_file(members, row), output, length, 1);
    }
    close_file(members);
    close_file(dir);
    finish(output, "program.gamma", 0);
}

int construct(long argc, const char *argv[]) {
    operation = "startup";
    if (argc > 1 && !compare(argv[1], "--alpha")) {
        target_alpha = 1;
        ++argv;
        --argc;
    }
    int delta = argc > 1 && !compare(argv[1], "--delta");
    if (delta) { ++argv; --argc; }
    long with_request = delta ? 4 : 5;
    if (argc < 3 || argc > with_request)
        fail("usage: bootstrap [--alpha] SOURCE_ROOT NEW_DIRECTORY [GAMMA_SOURCE [PRODUCER_INPUT]]"
             " or bootstrap [--alpha] --delta SOURCE_ROOT NEW_DIRECTORY [PRODUCER_INPUT]");
    int customer = !delta && argc >= 4 ? open_input(AT_FDCWD, argv[3]) : -1;
    int request = argc == with_request ? open_input(AT_FDCWD, argv[argc-1]) : -1;
    repository = checked(kernel(__NR_openat, AT_FDCWD, argv[1],
                                O_RDONLY | O_DIRECTORY | O_CLOEXEC), "cannot open source root");
    checked(kernel(__NR_mkdir, argv[2], 0700), "cannot create new output directory");
    checked(kernel(__NR_chdir, argv[2]), "cannot enter output directory");

    if (delta) pack_delta();
    if (customer >= 0) snapshot_input(customer, "program.gamma");
    if (request >= 0) snapshot_input(request, "program.input");

    int fd = create("admitted-beta.tape");
    write_all(fd, beta_root, (uintptr_t)beta_end - (uintptr_t)beta_root);
    close_file(fd);
    stamp("admitted-beta.tape", "beta-seed");
    run("./beta-seed", open_input(repository, "bootstrap/beta/beta_compiler.beta"),
        "candidate-beta.tape", 0);
    require_equal("admitted-beta.tape", "candidate-beta.tape");
    checked(kernel(__NR_rename, "candidate-beta.tape", "beta.tape"), "cannot admit Beta");
    stamp("beta.tape", "beta");
    run("./beta", open_input(repository, "bootstrap/gamma/interpreter/gamma_evaluator.beta"),
        "gamma.tape", 0);
    stamp("gamma.tape", "gamma-evaluator");
    prepare_source();
    compile_stage("./gamma-evaluator", "compiler.request", "gamma0", "gamma0.tape");
    compile_stage("./gamma0", "compiler.gamma", "gamma1", "gamma1.tape");
    require_equal("gamma0", "gamma1");
    compile_stage("./gamma1", "compiler.gamma", "gamma2", "gamma2.tape");
    require_equal("gamma1", "gamma2");
    checked(kernel(__NR_rename, "gamma2", "gamma-compiler"), "cannot publish compiler");
    if (delta || customer >= 0)
        compile_stage("./gamma-compiler", "program.gamma", "program", "program.tape");
    if (request >= 0) {
        run("./program", open_input(AT_FDCWD, "program.input"), "producer.output", 0);
        compile_stage("./gamma-compiler", "producer.output", "successor", "successor.tape");
    }
    return 0;
}
