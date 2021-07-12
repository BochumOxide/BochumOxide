# Build
In order to build this repository, you first need to build a static library of the Unicorn engine.
Unicorn is used to add symbols to PLT stubs in ELF binaries.
Unicorn Rust bindings have been added only on the `next` branch which has ~~a UAF bug that needs~~ two UAF bugs that need to be fixed first.
The fixes can be found in the `patch.diff` file.

Initialize submodule:
```
git submodule update --init
```

Apply bug fixes:
```
cp patch.diff unicorn/
cd unicorn
git apply patch.diff
```

Build Unicorn (must use cmake build because standard Makefile build seems to be broken):
```
mkdir build
cd build
cmake ..
make
```

Generate static library:
```
cd ..
UNICORN_ARCHS="arm aarch64 x86" ./make.sh
```

Now you should be able to just build the entire project by executing `cargo build` from the root directory of the repository.

# Debugging memory corruptions
Since by now I already had to fix two memory corruption bugs in the Unicorn engine, here is a short introduction on how to spot them in Rust builds.

First, switch to the `nightly` toolchain:
```
rustup override set nightly
```

Then, Rust can be enabled to use Clang's `AddressSanitizer` which helps to find memory bugs.
You can make use of the sanitizers by executing the code like this:
```
export RUSTFLAGS=-Zsanitizer=address RUSTDOCFLAGS=-Zsanitizer=address
cargo run -Zbuild-std --target x86_64-unknown-linux-gnu
```

As one of the bug fixes is merely a hotfix which inherently causes memory leaks, you might want to ignore leaks like this:
```
ASAN_OPTIONS=detect_leaks=0 cargo run -Zbuild-std --target x86_64-unknown-linux-gnu
```

If ASAN is not able to correctly resolve symbols in the call traces, make sure that `llvm-symbolizer` runs on verison 10.0.0.

When you want to switch back to `stable` builds, run the following commands:
```
unset RUSTDOCFLAGS
unset RUSTFLAGS
rustup override set stable
```

Now you should be able to execute `cargo build`/`cargo run` as usual.
