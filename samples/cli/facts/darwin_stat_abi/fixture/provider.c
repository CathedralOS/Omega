// On Darwin this exchanges the SDK's actual struct stat. Other hosts exercise
// the same explicit Darwin format, not their unrelated native struct stat.
#define _DARWIN_C_SOURCE 1
#include <stdint.h>
#include <stddef.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#if defined(__APPLE__)
#include <sys/stat.h>
typedef struct stat fixture_stat;
typedef struct timespec fixture_timespec;
_Static_assert(UTIME_NOW == -1 && UTIME_OMIT == -2, "Darwin UTIME sentinel values");
#else
typedef struct fixture_timespec { int64_t tv_sec; int64_t tv_nsec; } fixture_timespec;
typedef struct fixture_stat {
    int32_t st_dev; uint16_t st_mode; uint16_t st_nlink; uint64_t st_ino;
    uint32_t st_uid; uint32_t st_gid; int32_t st_rdev;
    fixture_timespec st_atimespec, st_mtimespec, st_ctimespec, st_birthtimespec;
    int64_t st_size, st_blocks; int32_t st_blksize;
    uint32_t st_flags, st_gen; int32_t st_lspare; int64_t st_qspare[2];
} fixture_stat;
#endif
#if defined(_WIN32)
#define EXPORTED __declspec(dllexport)
#else
#define EXPORTED __attribute__((visibility("default")))
#endif
_Static_assert(sizeof(fixture_stat)==144, "Darwin stat complete extent");
_Static_assert(_Alignof(fixture_stat)==8, "Darwin stat alignment");
#define OFFSET(field, expected) _Static_assert(offsetof(fixture_stat, field)==expected, #field " offset")
OFFSET(st_dev, 0);
OFFSET(st_mode, 4);
OFFSET(st_nlink, 6);
OFFSET(st_ino, 8);
OFFSET(st_uid, 16);
OFFSET(st_gid, 20);
OFFSET(st_rdev, 24);
OFFSET(st_atimespec, 32);
OFFSET(st_mtimespec, 48);
OFFSET(st_ctimespec, 64);
OFFSET(st_birthtimespec, 80);
OFFSET(st_size, 96);
OFFSET(st_blocks, 104);
OFFSET(st_blksize, 112);
OFFSET(st_flags, 116);
OFFSET(st_gen, 120);
OFFSET(st_lspare, 124);
OFFSET(st_qspare, 128);
_Static_assert(sizeof(((fixture_stat*)0)->st_atimespec)==16, "complete timespec");
_Static_assert(sizeof(((fixture_stat*)0)->st_dev)==4, "signed 32-bit device");
_Static_assert((( __typeof__(((fixture_stat*)0)->st_dev))-1)<0, "device signedness");
_Static_assert(sizeof(((fixture_stat*)0)->st_qspare)==16, "complete reserved tail");

EXPORTED void darwin_stat_fixture_fill(fixture_stat *value) {
    if (!value) abort();
    memset(value, 0, sizeof(*value));
    value->st_dev = INT32_C(-2147483647);
    value->st_mode = UINT16_C(33188);
    value->st_nlink = UINT16_C(3);
    value->st_ino = UINT64_C(4294967298);
    value->st_uid = UINT32_C(123);
    value->st_gid = UINT32_C(456);
    value->st_rdev = INT32_C(-123456789);
    value->st_atimespec.tv_sec = INT64_C(-17);
    value->st_atimespec.tv_nsec = INT64_C(123456789);
    value->st_mtimespec.tv_sec = INT64_C(42);
    value->st_mtimespec.tv_nsec = INT64_C(987654321);
    value->st_ctimespec.tv_sec = INT64_C(-9);
    value->st_ctimespec.tv_nsec = INT64_C(111111111);
    value->st_birthtimespec.tv_sec = INT64_C(77);
    value->st_birthtimespec.tv_nsec = INT64_C(222222222);
    value->st_size = INT64_C(8589934593);
    value->st_blocks = INT64_C(123456);
    value->st_blksize = INT32_C(4096);
    value->st_flags = UINT32_C(2147483649);
    value->st_gen = UINT32_C(2882400001);
    value->st_lspare = INT32_C(-314159);
    value->st_qspare[0] = INT64_C(-4294967297);
    value->st_qspare[1] = INT64_C(68719476737);
}

EXPORTED uint64_t darwin_stat_fixture_check(const fixture_stat *value) {
    if (!value) return UINT64_C(1);
    if (value->st_dev != INT32_C(-2147483647)) return UINT64_C(2);
    if (value->st_mode != UINT16_C(33188)) return UINT64_C(3);
    if (value->st_nlink != UINT16_C(3)) return UINT64_C(4);
    if (value->st_ino != UINT64_C(4294967298)) return UINT64_C(5);
    if (value->st_uid != UINT32_C(123)) return UINT64_C(6);
    if (value->st_gid != UINT32_C(456)) return UINT64_C(7);
    if (value->st_rdev != INT32_C(-123456789)) return UINT64_C(8);
    if (value->st_atimespec.tv_sec != INT64_C(-17)) return UINT64_C(9);
    if (value->st_atimespec.tv_nsec != INT64_C(123456789)) return UINT64_C(10);
    if (value->st_mtimespec.tv_sec != INT64_C(42)) return UINT64_C(11);
    if (value->st_mtimespec.tv_nsec != INT64_C(987654321)) return UINT64_C(12);
    if (value->st_ctimespec.tv_sec != INT64_C(-9)) return UINT64_C(13);
    if (value->st_ctimespec.tv_nsec != INT64_C(111111111)) return UINT64_C(14);
    if (value->st_birthtimespec.tv_sec != INT64_C(77)) return UINT64_C(15);
    if (value->st_birthtimespec.tv_nsec != INT64_C(222222222)) return UINT64_C(16);
    if (value->st_size != INT64_C(8589934593)) return UINT64_C(17);
    if (value->st_blocks != INT64_C(123456)) return UINT64_C(18);
    if (value->st_blksize != INT32_C(4096)) return UINT64_C(19);
    if (value->st_flags != UINT32_C(2147483649)) return UINT64_C(20);
    if (value->st_gen != UINT32_C(2882400001)) return UINT64_C(21);
    if (value->st_lspare != INT32_C(-314159)) return UINT64_C(22);
    if (value->st_qspare[0] != INT64_C(-4294967297)) return UINT64_C(23);
    if (value->st_qspare[1] != INT64_C(68719476737)) return UINT64_C(24);
    return UINT64_C(0);
}
EXPORTED uint64_t darwin_stat_fixture_times(const fixture_timespec *value) {
    if (!value) return UINT64_C(1);
    return value[0].tv_sec == INT64_C(-17) && value[0].tv_nsec == INT64_C(-2)
        && value[1].tv_sec == INT64_C(42) && value[1].tv_nsec == INT64_C(-1)
        && sizeof(fixture_timespec[2]) == 32 && _Alignof(fixture_timespec[2]) == 8
        ? UINT64_C(0) : UINT64_C(1);
}
EXPORTED uint64_t darwin_stat_fixture_size(void) { return sizeof(fixture_stat); }
EXPORTED uint64_t darwin_stat_fixture_alignment(void) { return _Alignof(fixture_stat); }

#ifdef DARWIN_STAT_FIXTURE_TEST
int main(void) {
    fixture_timespec times[2] = {{INT64_C(-17), INT64_C(-2)}, {INT64_C(42), INT64_C(-1)}};
    if (darwin_stat_fixture_times(times)!=0) return 25;
    times[1].tv_nsec=0;
    if (darwin_stat_fixture_times(times)==0) return 26;
    fixture_stat value;
    darwin_stat_fixture_fill(&value);
    if (darwin_stat_fixture_check(&value)!=0) return 1;
    // Every authored field must be observed by the C-side checker.
    { fixture_stat changed=value; changed.st_dev = 0; if (darwin_stat_fixture_check(&changed)==0) return 2; }
    { fixture_stat changed=value; changed.st_mode = 0; if (darwin_stat_fixture_check(&changed)==0) return 3; }
    { fixture_stat changed=value; changed.st_nlink = 0; if (darwin_stat_fixture_check(&changed)==0) return 4; }
    { fixture_stat changed=value; changed.st_ino = 0; if (darwin_stat_fixture_check(&changed)==0) return 5; }
    { fixture_stat changed=value; changed.st_uid = 0; if (darwin_stat_fixture_check(&changed)==0) return 6; }
    { fixture_stat changed=value; changed.st_gid = 0; if (darwin_stat_fixture_check(&changed)==0) return 7; }
    { fixture_stat changed=value; changed.st_rdev = 0; if (darwin_stat_fixture_check(&changed)==0) return 8; }
    { fixture_stat changed=value; changed.st_atimespec.tv_sec = 0; if (darwin_stat_fixture_check(&changed)==0) return 9; }
    { fixture_stat changed=value; changed.st_atimespec.tv_nsec = 0; if (darwin_stat_fixture_check(&changed)==0) return 10; }
    { fixture_stat changed=value; changed.st_mtimespec.tv_sec = 0; if (darwin_stat_fixture_check(&changed)==0) return 11; }
    { fixture_stat changed=value; changed.st_mtimespec.tv_nsec = 0; if (darwin_stat_fixture_check(&changed)==0) return 12; }
    { fixture_stat changed=value; changed.st_ctimespec.tv_sec = 0; if (darwin_stat_fixture_check(&changed)==0) return 13; }
    { fixture_stat changed=value; changed.st_ctimespec.tv_nsec = 0; if (darwin_stat_fixture_check(&changed)==0) return 14; }
    { fixture_stat changed=value; changed.st_birthtimespec.tv_sec = 0; if (darwin_stat_fixture_check(&changed)==0) return 15; }
    { fixture_stat changed=value; changed.st_birthtimespec.tv_nsec = 0; if (darwin_stat_fixture_check(&changed)==0) return 16; }
    { fixture_stat changed=value; changed.st_size = 0; if (darwin_stat_fixture_check(&changed)==0) return 17; }
    { fixture_stat changed=value; changed.st_blocks = 0; if (darwin_stat_fixture_check(&changed)==0) return 18; }
    { fixture_stat changed=value; changed.st_blksize = 0; if (darwin_stat_fixture_check(&changed)==0) return 19; }
    { fixture_stat changed=value; changed.st_flags = 0; if (darwin_stat_fixture_check(&changed)==0) return 20; }
    { fixture_stat changed=value; changed.st_gen = 0; if (darwin_stat_fixture_check(&changed)==0) return 21; }
    { fixture_stat changed=value; changed.st_lspare = 0; if (darwin_stat_fixture_check(&changed)==0) return 22; }
    { fixture_stat changed=value; changed.st_qspare[0] = 0; if (darwin_stat_fixture_check(&changed)==0) return 23; }
    { fixture_stat changed=value; changed.st_qspare[1] = 0; if (darwin_stat_fixture_check(&changed)==0) return 24; }
    printf("stat size=%zu align=%zu; checked23 fields\n", sizeof(value), _Alignof(fixture_stat));
#if defined(__APPLE__)
    puts("uses actual Darwin SDK struct stat");
#else
    puts("uses explicit Darwin format replica; no native Darwin SDK claim");
#endif
    return 0;
}
#endif
