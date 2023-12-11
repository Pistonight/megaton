#pragma once

#include "common.hpp"
// these should be defined
#ifndef EXL_DEBUG
#define EXL_DEBUG
#endif
#ifndef EXL_USE_FAKEHEAP
#define EXL_USE_FAKEHEAP
#endif

#ifndef EXL_HEAP_SIZE
#define EXL_HEAP_SIZE 0x5000
#endif

#ifndef EXL_JIT_SIZE
#define EXL_JIT_SIZE 0x1000
#endif

#ifndef EXL_INLINE_POOL_SIZE
#define EXL_INLINE_POOL_SIZE 0x1000
#endif

/*
#define EXL_SUPPORTS_REBOOTPAYLOAD
*/

namespace exl::setting {
    /* How large the fake .bss heap will be. */
    constexpr size_t HeapSize = EXL_HEAP_SIZE;

    /* How large the JIT area will be for hooks. */
    constexpr size_t JitSize = EXL_JIT_SIZE;

    /* How large the area will be inline hook pool. */
    constexpr size_t InlinePoolSize = EXL_INLINE_POOL_SIZE;

    /* Sanity checks. */
    static_assert(ALIGN_UP(JitSize, PAGE_SIZE) == JitSize, "JitSize is not aligned");
    static_assert(ALIGN_UP(InlinePoolSize, PAGE_SIZE) == JitSize, "InlinePoolSize is not aligned");
}
