#include <megaton.hpp>

extern "C" __attribute__((noreturn)) void megaton_default_abort(int code) {
    // Credit: exlaunch/source/lib/diag/abort.cpp
    register s64 addr __asm__("x27") = 0x6969696969696969;
    register s64 val __asm__("x28")  = code;
    while (true) {
        __asm__ __volatile__ (
                "str %[val], [%[addr]]"
                :
                : [val]"r"(val), [addr]"r"(addr)
            );
    }
}
