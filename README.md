
for faster compilation, add this to `./cargo/config.toml`:
```
[target.x86_64-unknown-linux-gnu]
# follow the instructions on https://github.com/rui314/mold
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=/usr/local/bin/mold"]
```

See all the binaries in tests/src/bin

To create a consumer chain,

1. Make sure onomy_tests, manual_tests, onomy, the multiverse main branch, and the repo containing whatever module (In this example I will use the market module) are all up to date with each other. In the Cargo.tomls there are `git`+`rev` dependencies that may need to be updated, and there is `dockerfiles.rs` and places where the latest versions of things are defined. You can run things like `rustup update`, and in each repo `cargo update`, and `cargo clean` to make sure Rust is good. The multiverse repo has some additional information about Cosmos-SDK versions and updating things like OpenAPI.
2. The module source repo like the market repo usually has its own standalone cosmos binary that can be built and tested, you may want to add a standalone test to onomy_tests (e.x. `market_standalone`) to make sure you are familiar with it and put in some basic tests to make sure CLI works before bringing in the complications of ICS. You may need to dig in to setups.rs and other things depending on what special functionality is brought in by the module(s).
3. Create something like a `onex` and `onex-dev` branch pair on the multiverse repo from the main branch, importing the modules you need, updating app.toml and customizing the chain_ids and default home directories and the openapi etc.
4. Get the `ics_with_onomyd.rs` to complete, adding any specializations from the standalone to make sure the CLI still works. By this point, you can look at the default genesis files being generated and create a partial genesis and consumer addition proposal, again see the extra documentation on the multiverse repo.
5. Tag and create a new version, add it to `dockerfiles.rs` and double check all the versions being used.
6. Use reparse_accounts.rs to get a partial genesis, if you are going the route where governance will use a coin with accounts matching the bonded amounts on the provider. Typically we are putting the files in the environments repo, until we get a complete genesis which should be put on its own branch in the multiverse repo for the public
7. 
