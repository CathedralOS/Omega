// Static fixture for the checked program-launch host, including denied resources.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc < 2) return 1;
    if (!strcmp(argv[1], "143")) return 143;
    if (!strcmp(argv[1], "signal")) return raise(SIGTERM);
    if (!strcmp(argv[1], "never")) for (;;) sleep(1);
    if (!strcmp(argv[1], "descendant")) {
        int ready[2];
        if (pipe(ready) || argc != 3) return 7;
        pid_t child = fork();
        if (child < 0) return 8;
        if (child == 0) {
            close(ready[0]);
            if (setsid() < 0 || write(ready[1], "R", 1) != 1) return 9;
            for (;;) sleep(1);
        }
        char byte;
        close(ready[1]);
        return read(ready[0], &byte, 1) == 1 && byte == 'R' ? 19 : 10;
    }
    if (argc != 4 || strcmp(argv[0], "program") || strcmp(argv[1], "probe")
        || strcmp(argv[2], "") || strcmp(argv[3], "two words")) return 2;

    if (environ[0] || fopen("/etc/passwd", "r") || socket(AF_INET, SOCK_STREAM, 0) >= 0)
        return 3;
    for (int fd = 3; fd < 64; fd++)
        if (fcntl(fd, F_GETFD) != -1 || errno != EBADF) return 4;

    FILE *input = fopen("in", "r");
    if (!input || fgetc(input) != 'A' || fclose(input) || fopen("in", "w")) return 5;
    FILE *output = fopen("nested/out", "w");
    if (!output || fputc('B', output) == EOF || fclose(output)) return 6;
    return 17;
}
