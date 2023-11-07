
for faster compilation, add this to `./cargo/config.toml`:
```
[target.x86_64-unknown-linux-gnu]
# follow the instructions on https://github.com/rui314/mold
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=/usr/local/bin/mold"]
```

To run TheGraph, run `cargo r --bin query_graph -- --peer-info ...`. See the comments in
`tests/src/bin/query_graph.rs` for more info and options
