#pragma once
/*
 * megaton.hpp
 *
 * Symbols present in the megaton runtime.
 *
 * These symbols are either specific to the megaton framework,
 * or provided by the devkitA64 toolchain.
 */

#include <megaton/types.h>

namespace megaton {

/// Struct that holds module name info.
///
/// This is defined in Rust with the #[megaton::module] macro.
class ModuleName {
public:
    int _unknown;
    /// Not constructable
    ModuleName() = delete;
private:
    int _len;
    /// There are more chars, but since we don't know the length
    /// on the C side, we can't access them.
    char first_char;

    /// Get the length of the module name.
    int len() const {
        return _len;
    }

    /// Get the name of the module. The strings is null-terminated.
    const char* name() const {
        return &first_char;
    }
};

}

extern "C" {
/// Module name getter bootstrapped on Rust side
extern const megaton::ModuleName* megaton_module_name();

/// Main function bootstraped on the Rust side
extern void megaton_rust_main();

/// The start of executable (defined in linker script/main.s)
extern char __module_start;


/* /// Module entrypoint called by rtld */
/* extern void megaton_entrypoint(void); */
/*  */
/* /// Module main function, defined with the #[megaton::main] macro */
/* extern void megaton_main(); */

/// Module entrypoint called by rtld
void megaton_entrypoint(void);

}


