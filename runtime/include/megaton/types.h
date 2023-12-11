#pragma once

#include <switch/types.h>

typedef unsigned char   uchar;
typedef	unsigned short	ushort;
typedef	unsigned int	uint;	
typedef	unsigned long	ulong;

#define attr_noinline_ __attribute__((noinline))
#define attr_noreturn_ __attribute__((noreturn))
#define attr_inline_   __attribute__((always_inline))
