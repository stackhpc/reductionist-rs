# Adding support for a new compression algorithm

The Reductionist supports decompression of compressed object data chunks.

Reductionist tries to make it as easy as possible to add support for new compression algorithms.

Reductionist is built using [Rust](https://rust-lang.org/) and prerequisite knowledge will be invaluable.

A good source of Rust knowledge can be found in what's affectionately named [the book](https://rust-lang.org/learn/), which is available to read for free online.

Rust is very good at ensuring performant code is written, at the same time this can seem a little unforgiving.

There are built-in tools we will use whilst writing this code:
* `cargo fmt` - ensures the code conforms to code formatting standards aiding in readability and maintainability
* `cargo clippy` - a collection of [lints](https://en.wikipedia.org/wiki/Lint_(software)) that catch common mistakes and improve on code
* `cargo test` - built-in unit testing aiming for complete code test coverage

These tools are incorporated into the Reductionist repository's commit procedure, it's not possible to merge a pull request for new compression support into the `main` branch without these tools passing.

We'll document how these tools can be run locally from a single script to ensure there are no surprises when the repository workflow runs these same tools.

## Explaining by example

Arguably the easiest way to add support for a new compression algorithm is to follow an example, using it as a template.

We will follow support for the addition of [Blosc2](https://blosc.org/).

This example has been contributed to the main [Reductionist repository](https://github.com/stackhpc/reductionist-rs/) and can be found in its Git history.

The process of contributing to the Reductionist involves a pull request which for this example can be found [here](https://github.com/stackhpc/reductionist-rs/pull/116). This PR gives a nice view of the changes required to add new compression support.


## Prevent reinventing the wheel with Rust crates

If we're to add support for a new compression algorithm we should first check [crates.io: Rust Package Registry](https://crates.io/).
This houses well tested implementations of community provided Rust software.

If we're to provide support for Blosc2 we should see if a compression / decompression crate already exists so we don't have to write our own.

Something to consider when looking for existing crates is "Are they implemented purely in Rust or are they Rust bindings to existing system libraries?"

**We prefer a pure Rust implementation, because if we're to deploy Reductionist we already have everything "Rust" covered.**

**Incorporating a potentially non-standard system library into the Reductionist deployment is not covered and out of scope.**

### Choosing a Blosc2 crate

We go to [https://crates.io/](https://crates.io/) where we can search for *Blosc2*.

Choosing a crate was actually a bit of an iterative process, there are many crates supporting Blosc2.

Documentation varies across crates and selecting the most popular and supported is a good idea, but the most popular were found to be Rust bindings around the [c-blosc2 library](https://github.com/Blosc/c-blosc2).

When a crate is added to your project it will be automatically downloaded, along with any dependencies, and built.
This build actually failed on an Ubuntu system, not a good sign.
Getting past the build failures you still need to ensure with a containerised Reductionist [deployment](https://github.com/stackhpc/reductionist-rs/blob/main/docs/deployment.md) that the library is also present.

The easiest way to obtain support for Blosc2 is through a pure Rust implementation.

The closest we can get to this is [blusc](https://crates.io/crates/blusc), its addition to the dependency tree includes zlib which is an acceptable system library usually installed by default.

## Updating the Reductionist code base

Assumptions made are:
* You know how to obtain the [Reductionist code](https://github.com/stackhpc/reductionist-rs)
* You are familiar with developing new features in a [feature branch of the repository](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/about-branches)
* You have an understanding of [Rust](https://rust-lang.org/)
* You have an understanding of [cargo](https://doc.rust-lang.org/cargo/) - Rust's build system and package manager
* You have an understanding of [GitHub and pull requests](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/about-pull-requests)

### Add the blusc crate to the Reductionist

Starting point - you've checked out the Reductionist and are within the root of the repository.

Add the `blusc` dependency to Reductionist:

```shell
cargo add blusc
```

If you run `git status` you'll notice this has updated `Cargo.toml`.

A nice touch is to edit `Cargo.toml` where you'll see the `blusc` dependency has been added:

```toml
blusc = "0.0.2"
```

Then you move this dependency so they're listed alphabetically, nice if you're ever managing dependencies by hand.

### Update the list of compression algorithms available - src/models.rs

Update `src/models.rs`

This contains the Compression [enum](https://doc.rust-lang.org/rust-by-example/custom_types/enum.html), essentially the list of compression algorithms supported by the Reductionist.

The order isn't important so Blosc2 was inserted first into the list to keep it alphabetical.

#### Testing in models.rs

The unit test `test_invalid_compression` in `src/models.rs` must be updated.

Requests made to the Reductionist will name a compression algorithm, if used, so we're just making sure the validation for requests is in sync with the algorithms listed in the enum.

### Add the code behind the (Blosc2) decompression - src/compression.rs

This file contains the main body of the update.

I'm going to mention what you probably already know when it comes to Rust and `cargo clippy`, but it's nice to reiterate.

There's a [match](https://doc.rust-lang.org/rust-by-example/flow_control/match.html) pattern that determines the function used for each compression algorithm listed in the `Compression` enum we updated earlier in `src/models.rs`.

Rust is not going to let our code compile unless all match branches are covered, Rust makes it impossible to leave this as a "loose end".

The [blusc documentation](https://docs.rs/blusc/0.0.2/blusc/) provides a quick start which is enough information to implement a decompression function.

You might well wonder whether you've implemented this correctly, well we're going to test it!

#### Testing in compression.rs

There are already unit tests in `src/compression.rs` for other compression algorithms, just use these as a template and add the equivalent tests for Blosc2.

A function is needed to provide compressed data, again the `blusc` documentation gives the information needed to implement this function.

We have a test that compresses data then decompresses to ensure the result matches initial input.

We also have a test that feeds invalid data into the decompression function we wrote, we expect this to throw an error so we need to manage how that error is handled.

### Reporting errors resulting from decompression - src/error.rs

Update `src/error.rs` which details how an error thrown by the Rust crate we've used is going to be reported.

In `compression.rs` we may simply propagate crate errors and in `src/error.rs` we can translate these to our native `ActiveStorageError` type.

There are plenty of different errors managed in `src/error.rs` such that this should be a copy and paste exercise.

We have a test that checks the formatting of an error message as reported by the API response.

### Benchmarking - benches/compression.rs

This can be easily missed, running benchmarks on Reductionist code is completely optional.

`cargo clippy` however won't miss us failing to update a `match` with a branch for the Blosc2 `enum` added.

We'll see later that `cargo clippy --all-targets` is needed to catch this error.

Update `benches/compression.rs` using other compression algorithms as an example template.

## Compiling the code

You should now be able to compile the code with:

```shell
cargo build
```

What I haven't mentioned in detail are the [use](https://doc.rust-lang.org/rust-by-example/mod/use.html) declarations needed to pull in relevant code dependencies from the `blusc` crate.

Rust is very good at pointing out what's missing, the crate documentation should also tell you what's needed.

Rust is also very good at keeping things tidy, pointing out where `use` is specifying unused dependencies, you can be liberal as you get the code working and remove the unused when later warned.

## Testing the code

We must have all tests passing if we're to merge this code into Reductionist's *main* branch.

Run:

```shell
cargo test
```

Test code is not built by default so you may now see new build errors.

## Running benchmarks

This is useful as it's also providing additional testing.

Run:

```shell
cargo bench
```

The benchmark code is not built by default so you may now see new build errors.

## Checking all prerequisites before committing code

### Code formatting

Run:

```shell
cargo fmt
```

to apply formatting changes suggested by `rustfmt`.

### Linting and suggested code improvements

Run:

```shell
cargo clippy
```

to see if it spots any commonly made coding errors.

As mentioned earlier, to run clippy on all code including tests and benchmarks you need to run:

```shell
cargo clippy --all-targets -- -D warnings
```

which also treats warnings as errors.

We've not made a big code change so if there are any they can probably be fixed with:

```shell
cargo clippy --fix --lib -p reductionist
```

As with everything cargo the output is very helpful and usually tells you what to run to have cargo fix things for you.

### Automating the whole sequence of checks

From the root directory of the Reductionist repository run:

```
./tools/pre-commit
```

This runs all of the above and must exit without error otherwise the Reductionist's GitHub workflow is going to find the same errors and prevent a pull request from being merged into the Reductionist `main` branch.

### Update the API documentation and schema - docs/api.md

Whenever you add a new `Compression` variant (such as `Blosc2`), you must also update the public API documentation and any associated schema definitions.

In particular, ensure that the `compression.id` field in `docs/api.md` (and any relevant schema) lists the new algorithm identifier (for example, `blosc2`) alongside the existing values (such as `gzip` and `zlib`), so that the documented API matches the behavior implemented in `src/models.rs`.

Keeping these documents in sync avoids confusing clients and prevents validation errors when they legitimately use newly supported compression algorithms.

## Completing the addition of a new compression algorithm

All changes to the Reductionist must be made in a feature branch and merged into the default `main` branch using a pull request.

It will not be possible to merge the changes unless all tests pass within the workflow.