# MMC Core Android Build Makefile
#
# This Makefile simplifies the process of building the MMC core native
# library for Android targets. It uses cargo-ndk to manage the NDK
# toolchain configuration and handles the compilation of cdylib files
# for multiple ABI targets.
#
# Requirements:
# - Android NDK r24+ (set $ANDROID_NDK_HOME or $ANDROID_NDK_ROOT)
# - cargo-ndk: `cargo install cargo-ndk`
# - Rust toolchain: 1.75+
# - Android targets (installed via rustup):
#   rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

.PHONY: help build-arm64 build-armeabi build-x86_64 build-all copy-libs clean targets

help:
	@echo "MMC Core Android Build Makefile"
	@echo ""
	@echo "Available targets:"
	@echo "  targets        - Install required Rust targets"
	@echo "  build-arm64    - Build for arm64-v8a (most Android devices)"
	@echo "  build-armeabi  - Build for armeabi-v7a (older 32-bit devices)"
	@echo "  build-x86_64   - Build for x86_64 (emulators and some devices)"
	@echo "  build-all      - Build all three ABIs in release mode"
	@echo "  copy-libs      - Copy .so files into Android project jniLibs"
	@echo "  clean          - Clean all build artifacts"
	@echo ""
	@echo "Environment variables:"
	@echo "  ANDROID_NDK_HOME - Path to Android NDK (required)"
	@echo "  PROFILE          - debug or release (default: release)"

PROFILE ?= release

ifeq ($(PROFILE),release)
	BUILD_FLAG = --release
else
	BUILD_FLAG =
endif

targets:
	rustup target add aarch64-linux-android
	rustup target add armv7-linux-androideabi
	rustup target add i686-linux-android
	rustup target add x86_64-linux-android

build-arm64:
	cargo ndk --target arm64-v8a build $(BUILD_FLAG) -p mmc-core-uniffi

build-armeabi:
	cargo ndk --target armeabi-v7a build $(BUILD_FLAG) -p mmc-core-uniffi

build-x86_64:
	cargo ndk --target x86_64 build $(BUILD_FLAG) -p mmc-core-uniffi

build-all: build-arm64 build-armeabi build-x86_64
	@echo "Built all ABIs successfully"

copy-libs: build-all
	mkdir -p bindings/android/app/src/main/jniLibs/arm64-v8a
	mkdir -p bindings/android/app/src/main/jniLibs/armeabi-v7a
	mkdir -p bindings/android/app/src/main/jniLibs/x86_64
	cp target/aarch64-linux-android/$(PROFILE)/libmmc_core.so bindings/android/app/src/main/jniLibs/arm64-v8a/
	cp target/armv7-linux-androideabi/$(PROFILE)/libmmc_core.so bindings/android/app/src/main/jniLibs/armeabi-v7a/
	cp target/x86_64-linux-android/$(PROFILE)/libmmc_core.so bindings/android/app/src/main/jniLibs/x86_64/
	@echo "Copied native libraries to jniLibs/"

clean:
	cargo clean
	rm -rf bindings/android/app/src/main/jniLibs/*/*.so
