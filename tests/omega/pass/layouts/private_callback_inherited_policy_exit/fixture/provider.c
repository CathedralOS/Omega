#include <stdint.h>
#include <string.h>
uint64_t timer_register(const unsigned char *gate) {
    uint16_t selector, low, middle;
    uint32_t high, reserved;
    memcpy(&low,gate,2); memcpy(&selector,gate+2,2);
    memcpy(&middle,gate+6,2); memcpy(&high,gate+8,4); memcpy(&reserved,gate+12,4);
    if(selector!=0x1234 || gate[4]!=5 || gate[5]!=0x8e || reserved!=0x89abcdef) return 1;
    uintptr_t address=(uintptr_t)low|((uintptr_t)middle<<16)|((uintptr_t)high<<32);
    if(!address) return 2;
    return ((uint64_t (*)(uint64_t))address)(102)==51 ? 0 : 3;
}
