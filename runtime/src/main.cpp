#include <megaton.hpp>

#include <exl/lib.hpp>
// required for linking with exlaunch
extern "C" void exl_main(void* x0, void* x1) {
    /* Setup hooking enviroment. */
    exl::hook::Initialize();

    /* Install the hook at the provided function pointer. Function type is checked against the callback function. */
    /* StubCopyright::InstallAtFuncPtr(nn::oe::SetCopyrightVisibility); */

    /* Alternative install funcs: */
    /* InstallAtPtr takes an absolute address as a uintptr_t. */
    /* InstallAtOffset takes an offset into the main module. */

    /*
    For sysmodules/applets, you have to call the entrypoint when ready
    exl::hook::CallTargetEntrypoint(x0, x1);
    */

   // call your code ...
}
