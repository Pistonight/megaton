# Makefile for the Megaton Runtime staticlib

ifeq ($(strip $(DEVKITPRO)),)
$(error \
"DEVKITPRO not set. \
Please install devkitPro and set the DEVKITPRO environment variable to point to it. \
 See https://devkitpro.org/wiki/Getting_Started for details.\
")
endif
ifeq ($(strip $(MEGATON_HOME)),)
$(error \
"MEGATON_HOME not set. \
Please set the MEGATON_HOME environment variable to point to the root of your local Megaton repository. \
")
endif

# Root of the library, where this Makefile is.
ROOT := $(MEGATON_HOME)/toolchain/runtime


.PHONY: test


C = $(CURDIR)

test:
	echo $(MEGATON_HOME)
