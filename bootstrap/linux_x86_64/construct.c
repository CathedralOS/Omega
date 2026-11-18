/* Build the native Gamma compiler with its predecessors. Only those
   programs read source syntax. This driver moves bytes and checks exits. */
#include <asm/unistd.h>
#include <linux/errno.h>
#include <linux/fcntl.h>
#include <stddef.h>
#include <stdint.h>

extern long kernel(long number, ...);
extern const unsigned char alpha_prefix[], alpha_suffix[], beta_root[], beta_end[];

static int repository;
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
    copy_file(repository, "bootstrap/compiled_vm/compiler.gamma", fd);
    write_all(fd, "\n", 1);
    copy_file(repository, "bootstrap/compiled_vm/frontend.gamma", fd);
    write_all(fd, "\n", 1);
    copy_file(repository, "bootstrap/independent/native.gamma", fd);
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

int construct(long argc, const char *argv[]) {
    operation = "startup";
    if (argc < 3 || argc > 5)
        fail("usage: bootstrap SOURCE_ROOT NEW_OUTPUT_DIRECTORY [GAMMA_SOURCE [PRODUCER_INPUT]]");
    int customer = argc >= 4 ? open_input(AT_FDCWD, argv[3]) : -1;
    int request = argc == 5 ? open_input(AT_FDCWD, argv[4]) : -1;
    repository = checked(kernel(__NR_openat, AT_FDCWD, argv[1],
                                O_RDONLY | O_DIRECTORY | O_CLOEXEC), "cannot open source root");
    checked(kernel(__NR_mkdir, argv[2], 0700), "cannot create new output directory");
    checked(kernel(__NR_chdir, argv[2]), "cannot enter output directory");

    if (customer >= 0) snapshot_input(customer, "program.gamma");
    if (request >= 0) snapshot_input(request, "program.input");

    int fd = create("admitted-beta.tape");
    write_all(fd, beta_root, (uintptr_t)beta_end - (uintptr_t)beta_root);
    close_file(fd);
    stamp("admitted-beta.tape", "beta-seed");
    run("./beta-seed", open_input(repository, "bootstrap/interpreted/1_beta/beta_compiler.beta"),
        "candidate-beta.tape", 0);
    require_equal("admitted-beta.tape", "candidate-beta.tape");
    checked(kernel(__NR_rename, "candidate-beta.tape", "beta.tape"), "cannot admit Beta");
    stamp("beta.tape", "beta");
    run("./beta", open_input(repository, "bootstrap/interpreted/2_gamma/gamma_evaluator.beta"),
        "gamma.tape", 0);
    stamp("gamma.tape", "gamma-evaluator");
    prepare_source();
    run("./gamma-evaluator", open_input(AT_FDCWD, "compiler.request"), "gamma0", 1);
    run("./gamma0", open_input(AT_FDCWD, "compiler.gamma"), "gamma1", 1);
    require_equal("gamma0", "gamma1");
    run("./gamma1", open_input(AT_FDCWD, "compiler.gamma"), "gamma2", 1);
    require_equal("gamma1", "gamma2");
    checked(kernel(__NR_rename, "gamma2", "gamma-compiler"), "cannot publish compiler");
    if (argc >= 4)
        run("./gamma-compiler", open_input(AT_FDCWD, "program.gamma"), "program", 1);
    if (argc == 5) {
        run("./program", open_input(AT_FDCWD, "program.input"), "producer.output", 0);
        run("./gamma-compiler", open_input(AT_FDCWD, "producer.output"), "successor", 1);
    }
    return 0;
}
