#include <string>

#include <exl/common.hpp>

#ifndef EXL_MODULE_NAME
#error "EXL_MODULE_NAME not defined!"
#endif

constexpr const int ModuleNameLength = std::char_traits<char>::length(EXL_MODULE_NAME);

struct ModuleName {
    int unknown;
    int name_length;
    char name[ModuleNameLength + 1];
};

__attribute__((section(".nx-module-name")))
__attribute__((used))
const ModuleName s_ModuleName = {
    .unknown = 0, 
    .name_length = ModuleNameLength, 
    .name = EXL_MODULE_NAME
};
