#include <ffi/ffi.h>
#include <stdint.h>
#include <stdio.h>

typedef struct __attribute__((aligned(32))) {
    uint64_t first;
    uint64_t second;
    uint64_t third;
} AlignedRecord;

static AlignedRecord transform(AlignedRecord record) {
    AlignedRecord result = {record.third, record.first, record.second};
    return result;
}

int main(void) {
    ffi_type *members[] = {&ffi_type_uint64, &ffi_type_uint64, &ffi_type_uint64, NULL};
    ffi_type record_type = {sizeof(AlignedRecord), _Alignof(AlignedRecord), FFI_TYPE_STRUCT, members};
    ffi_type *arguments[] = {&record_type};
    ffi_cif interface;
    ffi_status status = ffi_prep_cif(&interface, FFI_DEFAULT_ABI, 1, &record_type, arguments);
    if (status != FFI_OK) return 2;
    AlignedRecord input = {17, 91, 43};
    AlignedRecord result = {0};
    void *values[] = {&input};
    ffi_call(&interface, FFI_FN(transform), &result, values);
    printf("size=%zu alignment=%zu metadata=%zu/%u result=%llu/%llu/%llu\n",
        sizeof(AlignedRecord), _Alignof(AlignedRecord), record_type.size, record_type.alignment,
        (unsigned long long)result.first, (unsigned long long)result.second,
        (unsigned long long)result.third);
    return !(result.first == 43 && result.second == 17 && result.third == 91
        && input.first == 17 && input.second == 91 && input.third == 43);
}
