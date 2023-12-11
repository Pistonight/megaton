#pragma once
/*
 * Runtime assertion/panic handling.
 */

#include <megaton/types.h>

/// Format a panic message.
extern "C" const char* megaton_format_panic_message(const char* file, u32 line, const char* msg);

/// Bootstrapped on the Rust side
extern "C" {
/// Abort handler defined on Rust side
extern attr_noreturn_ void megaton_abort(int code);

/// Panic handler defined on Rust side
extern attr_noreturn_ void megaton_panic(const char* msg);
}

/* Assertion macros */
#define assert_(expr) \
    do { \
        if (!static_cast<bool>(expr)) { \
            megaton_panic(megaton_format_panic_message(__FILE__, __LINE__, "Assertion failed: " #expr)); \
        } \
    } while (0)

#define panic_(msg) \
    do { \
        megaton_panic(megaton_format_panic_message(__FILE__, __LINE__, msg)); \
    } while (0)

#define unreachable_() panic_("unreachable")
    
