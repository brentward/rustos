ROOT := $(shell git rev-parse --show-toplevel)

KERN := kernel
TARGET := target/aarch64-unknown-none/release/${KERN}
DEBUG_TARGET := target/aarch64-unknown-none/debug/${KERN}
# SDCARD ?= $(ROOT)/ext/fat32-imgs/mock1.fat32.img
SDCARD ?= $(ROOT)/user/fs.img
OBJCPY := cargo objcopy -- --strip-all -O binary
TTY_PATH := /dev/tty.SLAB_USBtoUART
# TTY_PATH := /dev/ttyUSB0
QEMU_ARGS ?=

.PHONY: all build qemu transmit objdump nm check clean install test

all: build

build:
	@echo "+ Building build/$(KERN).elf [xbuild/$@]"
	@cargo xbuild --release
	@mkdir -p build
	@cp -f $(TARGET) build/$(KERN).elf

	@echo "+ Building build/$(KERN).bin [objcopy]"
	@$(OBJCPY) $(TARGET) build/$(KERN).bin

debug:
	@echo "+ Building debug/$(KERN).elf [xbuild/$@]"
	@cargo xbuild
	@mkdir -p debug
	@cp -f $(DEBUG_TARGET) debug/$(KERN).elf

	@echo "+ Building debug/$(KERN).bin [objcopy]"
	@$(OBJCPY) $(DEBUG_TARGET) debug/$(KERN).bin

check:
	@cargo xcheck

qemu: build
	./qemu.sh build/$(KERN).bin -drive file=$(SDCARD),format=raw,if=sd $(QEMU_ARGS)

qemu-gdb: build
	./qemu.sh build/$(KERN).bin -drive file=$(SDCARD),format=raw,if=sd -s -S

qemu-debug: debug
	./qemu.sh debug/$(KERN).bin -drive file=$(SDCARD),format=raw,if=sd $(QEMU_ARGS)

qemu-gdb-debug: debug
	./qemu.sh debug/$(KERN).bin -drive file=$(SDCARD),format=raw,if=sd -s -S

qemu-asm: build
	./qemu.sh build/$(KERN).bin -drive file=$(SDCARD),format=raw,if=sd -d in_asm

transmit: build
	@echo "+ Transmitting build/$(KERN).bin to $(TTY_PATH)"
	ttywrite -i build/$(KERN).bin $(TTY_PATH)
	screen $(TTY_PATH) 115200

readobj: debug
	cargo readobj -- -file-headers debug/$(KERN).elf

size: debug
	cargo size --release -- -A debug/$(KERN).elf

objdump: debug
	cargo objdump -- -disassemble -no-show-raw-insn -print-imm-hex debug/$(KERN).elf

nm: build
	cargo nm build/$(KERN).elf

clean:
	cargo clean
	rm -rf build
	rm -rf debug

install: build
	@echo "+ Installing build/$(KERN).elf [install-kernel.py]"
	@$(ROOT)/bin/install-kernel.py build/$(KERN).elf

test:
	cargo test --target=$(shell $(ROOT)/bin/get-host-target.sh)
