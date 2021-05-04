# platform, options: x86_64
PLATFORM	=	x86_64
HOST		=  $(shell uname -s)
SCREEN_RESOLUTION =1280x768
# the path to OVMF.fd (for testing with EFI)
ifneq ("$(wildcard /usr/share/qemu/OVMF.fd)","")
    OVMF		=	/usr/share/qemu/OVMF.fd
else
	OVMF		=	./vendor/bootboot/OVMF-pure-efi-$(PLATFORM).fd
endif
# Options: debug, release. Make sure no space at end.
MODE		= debug
ifeq ($(MODE), release)
	CARGO_RELEASE_FLAG = --release
else
	CARGO_RELEASE_FLAG =
endif

KERNEL_SOURCES := $(shell find ./crates/supervisor/ -type f)
USERSPACE := $(wildcard ./crates/userspace/*)

.PHONY: userspace target/$(PLATFORM)-relic-kernel/$(MODE)/relic-kernel list

all: target/disk-$(PLATFORM)-$(MODE).img
run: efi

CARGO_STD_FEATURES=-Zbuild-std=core,compiler_builtins,alloc -Zbuild-std-features=compiler-builtins-mem

# List all targets
list:
	@$(MAKE) -pRrq -f $(lastword $(MAKEFILE_LIST)) : 2>/dev/null | awk -v RS= -F: '/^# File/,/^# Finished Make data base/ {if ($$1 !~ "^[#.]") {print $$1}}' | sort | egrep -v -e '^[^[:alnum:]]' -e '^$@$$'

# UserSpace build
userspace:
	cargo build $(CARGO_RELEASE_FLAG) --target ./triplets/$(PLATFORM)-relic-user.json --workspace --exclude relic-kernel $(CARGO_STD_FEATURES)

# Kernel build
target/$(PLATFORM)-relic-kernel/$(MODE)/relic-kernel: $(KERNEL_SOURCES)
	@mkdir ./target 2>/dev/null | true
	cargo build $(CARGO_RELEASE_FLAG) --target ./triplets/$(PLATFORM)-relic-kernel.json -p relic-kernel $(CARGO_STD_FEATURES)

# create an initial ram disk image with the kernel inside
target/disk-$(PLATFORM)-$(MODE).img: target/$(PLATFORM)-relic-kernel/$(MODE)/relic-kernel userspace
	@mkdir ./target/initrd ./target/initrd/sys ./target/initrd/sys ./target/initrd/userspace 2>/dev/null | true
	cp ./$< ./target/initrd/sys/core
	cd ./target/initrd/sys; echo -e "screen=$(SCREEN_RESOLUTION)\nkernel=sys/core\n" >config || true;
	cp $(USERSPACE:./crates/userspace/%=./target/$(PLATFORM)-relic-user/$(MODE)/%) ./target/initrd/userspace/
	./vendor/bootboot/mkbootimg-$(HOST) ./vendor/bootboot/mkimgconfig.json $@

check-image: target/$(PLATFORM)-relic-kernel/$(MODE)/relic-kernel
	./others/bootboot/mkbootimg-${HOST} check $^

efi: target/disk-$(PLATFORM)-$(MODE).img
	qemu-system-x86_64 -bios $(OVMF) -m 128 -drive file=./target/disk-x86_64-$(MODE).img,format=raw -serial stdio -no-shutdown -no-reboot

efi-monitor: target/disk-$(PLATFORM)-$(MODE).img
	qemu-system-x86_64 -bios $(OVMF) -m 128 -drive file=./target/disk-x86_64-$(MODE).img,format=raw -monitor stdio -serial vc

efi-wait: target/disk-$(PLATFORM)-$(MODE).img
	qemu-system-x86_64 -bios $(OVMF) -m 128 -drive file=./target/disk-x86_64-$(MODE).img,format=raw -serial vc -s -S

clean:
	rm -rf ./target
	cargo clean

test:
	cargo test --workspace --exclude relic-sigma

doc:
	cargo doc --target ./triplets/$(PLATFORM)-relic-kernel.json -p relic-kernel $(CARGO_STD_FEATURES)
	cargo doc --target ./triplets/$(PLATFORM)-relic-user.json --workspace --exclude relic-kernel $(CARGO_STD_FEATURES)