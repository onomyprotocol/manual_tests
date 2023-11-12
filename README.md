
for faster compilation, add this to `./cargo/config.toml`:
```
[target.x86_64-unknown-linux-gnu]
# follow the instructions on https://github.com/rui314/mold
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=/usr/local/bin/mold"]
```

See all the binaries in tests/src/bin
