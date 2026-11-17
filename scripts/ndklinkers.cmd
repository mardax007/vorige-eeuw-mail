@echo off
rem The NDK's compilers and linkers for plain cargo on Android targets (lint, the search speed
rem test on a phone). `tauri android build` sets these itself. Run inside androidenv.cmd, which
rem sets NDK_HOME:
rem     scripts\androidenv.cmd scripts\ndklinkers.cmd cargo clippy -p vem --target aarch64-linux-android
rem API level 24 is the app's minSdk.
set "TC=%NDK_HOME%\toolchains\llvm\prebuilt\windows-x86_64\bin"
set "CC_aarch64_linux_android=%TC%\aarch64-linux-android24-clang.cmd"
set "CXX_aarch64_linux_android=%TC%\aarch64-linux-android24-clang++.cmd"
set "AR_aarch64_linux_android=%TC%\llvm-ar.exe"
set "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=%TC%\aarch64-linux-android24-clang.cmd"
set "CC_x86_64_linux_android=%TC%\x86_64-linux-android24-clang.cmd"
set "CXX_x86_64_linux_android=%TC%\x86_64-linux-android24-clang++.cmd"
set "AR_x86_64_linux_android=%TC%\llvm-ar.exe"
set "CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=%TC%\x86_64-linux-android24-clang.cmd"
%*
