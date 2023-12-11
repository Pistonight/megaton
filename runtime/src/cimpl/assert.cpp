#include <cstdio>
#include <megaton/assert.hpp>

static char PANIC_FMT_BUFFER[1024];
extern "C" const char* megaton_format_panic_message(const char* file, u32 line, const char* msg) {
    snprintf(PANIC_FMT_BUFFER, sizeof(PANIC_FMT_BUFFER), "Panic at %s:%d:\n  %s", file, line, msg);
    // ensure termination
    PANIC_FMT_BUFFER[sizeof(PANIC_FMT_BUFFER) - 1] = '\0';
    return PANIC_FMT_BUFFER;
}
