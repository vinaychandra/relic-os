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

KERNEL_SOURCES := $(shell find ./crates/supervisor/ -type f)
USERSPACE := $(wildcard ./crates/userspace/*)

.PHONY: userspace target/$(PLATFORM)-relic-kernel/debug/relic-kernel

all: target/disk-$(PLATFORM).img
run: efi

CARGO_STD_FEATURES=-Zbuild-std=core,compiler_builtins,alloc -Zbuild-std-features=compiler-builtins-mem

# UserSpace build
userspace:
	cargo build --target ./triplets/$(PLATFORM)-relic-user.json --workspace --exclude relic-kernel $(CARGO_STD_FEATURES)

# Kernel build
target/$(PLATFORM)-relic-kernel/debug/relic-kernel: $(KERNEL_SOURCES)
	@mkdir ./target 2>/dev/null | true
	cargo build --target ./triplets/$(PLATFORM)-relic-kernel.json -p relic-kernel $(CARGO_STD_FEATURES)

# create an initial ram disk image with the kernel inside
target/disk-$(PLATFORM).img: target/$(PLATFORM)-relic-kernel/debug/relic-kernel userspace
	@mkdir ./target/initrd ./target/initrd/sys ./target/initrd/sys ./target/initrd/userspace 2>/dev/null | true
	cp ./$< ./target/initrd/sys/core
	cd ./target/initrd/sys; echo -e "screen=$(SCREEN_RESOLUTION)\nkernel=sys/core\n" >config || true;
	cp $(USERSPACE:./crates/userspace/%=./target/$(PLATFORM)-relic-user/debug/%) ./target/initrd/userspace/
	./vendor/bootboot/mkbootimg-$(HOST) ./vendor/bootboot/mkimgconfig.json $@

check-image: target/$(PLATFORM)-relic-kernel/debug/relic-kernel
	./others/bootboot/mkbootimg-${HOST} check $^

efi: target/disk-$(PLATFORM).img
	qemu-system-x86_64 -bios $(OVMF) -m 128 -drive file=./target/disk-x86_64.img,format=raw -serial stdio -no-shutdown -no-reboot

efi-monitor: target/disk-$(PLATFORM).img
	qemu-system-x86_64 -bios $(OVMF) -m 128 -drive file=./target/disk-x86_64.img,format=raw -monitor stdio -serial vc

efi-wait: target/disk-$(PLATFORM).img
	qemu-system-x86_64 -bios $(OVMF) -m 128 -drive file=./target/disk-x86_64.img,format=raw -serial vc -s -S

clean:
	rm -rf ./target
	cargo clean

test:
	cargo test --workspace --exclude relic-sigma

doc:
	cargo doc --target ./triplets/$(PLATFORM)-relic-kernel.json -p relic-kernel $(CARGO_STD_FEATURES)
	cargo doc --target ./triplets/$(PLATFORM)-relic-user.json --workspace --exclude relic-kernel $(CARGO_STD_FEATURES)