# Adding support for a new filter

The Reductionist supports decompression of compressed object data chunks.

To quote the documentation of [Rust crate byteshuffle](https://docs.rs/byteshuffle/latest/byteshuffle/)

> The byte-shuffle is a very efficient way to improve the compressibility of data that consists of an array of fixed-size objects. It rearranges the array in order to group all elements’ least significant bytes together, most-significant bytes together, and everything in between. Since real applications’ arrays often contain consecutive elements that are closely correlated with each other, this filter frequently results in lengthy continuous runs of identical bytes. Such runs are highly compressible by general-purpose compression libraries like gzip, lz4, etc.

In compression algorithms such as [Blosc](https://blosc.org/) the algorithm may incorporate their own filters to improve compression.

Where the filter isn't incorporated into the compression algorithm Reductionist allows filters to be incorporated into its pipeline, first the pipeline decompresses object data and then it reverse filters the data.

Reductionist tries to make it as easy as possible to add support for new filters.

Reductionist is built using [Rust](https://rust-lang.org/) and prerequisite knowledge will be invaluable.

A good source of Rust knowledge can be found in what's affectionately named [the book](https://rust-lang.org/learn/), which is available to read for free online.

Rust is very good at ensuring performant code is written, at the same time this can seem a little unforgiving.

There are built-in tools we will use whilst writing this code:
* `cargo fmt` - ensures the code conforms to code formatting standards aiding in readability and maintainability
* `cargo clippy` - a collection of [lints](https://en.wikipedia.org/wiki/Lint_(software)) that catch common mistakes and improve on code
* `cargo test` - built-in unit testing aiming for complete code test coverage

These tools are incorporated into the Reductionist repository's commit procedure, it's not possible to merge a pull request for a new filter into the `main` branch without these tools passing.

We'll document how these tools can be run locally from a single script to ensure there are no surprises when the repository workflow runs these same tools.

## Explaining by example

Arguably the easiest way to add support for a new filter is to follow an example, using it as a template.

We will follow support for the addition of [byteshuffle](https://docs.rs/byteshuffle/latest/byteshuffle/).

Reductionist already has deshuffle support, `byteshuffle` and its deshuffle component were inspired by the Blosc project and offer SIMD-accelerated byte shuffle/unshuffle routines.

Reductionist incorporates benchmarking which will allow a performance comparison of these two competing filters.

This example has been contributed to the main [Reductionist repository](https://github.com/stackhpc/reductionist-rs/) and can be found in its Git history.

The process of contributing to the Reductionist involves a pull request which for this example can be found [here](https://github.com/stackhpc/reductionist-rs/pull/117). This PR gives a nice view of the changes required to add new filter support.

## Prevent reinventing the wheel with Rust crates

If we're to add support for a new feature we should first check [crates.io: Rust Package Registry](https://crates.io/).
This houses well tested implementations of community provided Rust software.

We've already mentioned `byteshuffle`, this existing SIMD-accelerated byte shuffle/unshuffle implementation is a good example of using community tested code so we don't have to write our own.

Something to consider when looking for existing crates is "Are they implemented purely in Rust or are they Rust bindings to existing system libraries?"

**We prefer a pure Rust implementation, because if we're to deploy Reductionist we already have everything "Rust" covered.**

**Incorporating a potentially non-standard system library into the Reductionist deployment is not covered and out of scope.**

### Choosing an existing filter crate

We go to [https://crates.io/](https://crates.io/) where we can search for filters by keyword.

Choosing a crate can be a bit of an iterative process, there are often many available.

Documentation varies across crates and selecting the most popular and supported is a good idea.

When a crate is added to your project it will be automatically downloaded, along with any dependencies, and built.

If the build fails, is it a Rust compilation error or an indication that it builds against system libraries?

Implementations that are Rust bindings against system libraries should be avoided if possible. To ensure a straightforward containerised Reductionist [deployment](https://github.com/stackhpc/reductionist-rs/blob/main/docs/deployment.md) avoid crates that need non-standard libraries adding to Reductionist's container.

## Updating the Reductionist code base

Assumptions made are:
* You know how to obtain the [Reductionist code](https://github.com/stackhpc/reductionist-rs)
* You are familiar with developing new features in a [feature branch of the repository](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/about-branches)
* You have an understanding of [Rust](https://rust-lang.org/)
* You have an understanding of [cargo](https://doc.rust-lang.org/cargo/) - Rust's build system and package manager
* You have an understanding of [GitHub and pull requests](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/about-pull-requests)

### Add the byteshuffle crate to the Reductionist

Starting point - you've checked out the Reductionist and are within the root of the repository.

Add the `byteshuffle` dependency to Reductionist:

```shell
cargo add byteshuffle
```

If you run `git status` you'll notice this has updated `Cargo.toml`.

A nice touch is to edit `Cargo.toml` where you'll see the `byteshuffle` dependency has been added:

```toml
byteshuffle = "0.1.2"
```

Then you move this dependency so they're listed alphabetically, nice if you're ever managing dependencies by hand.

### Update the list of filters available - src/models.rs

Update `src/models.rs`

This contains the Filter [enum](https://doc.rust-lang.org/rust-by-example/custom_types/enum.html), essentially the list of filters supported by the Reductionist.

The order isn't important so we try to keep it alphabetical.

We add a new enum to the list of filters.

We might want to name the enum variant as `Shuffle_SIMD` for two reasons:

* SIMD is an acronym, and we want to reflect that in the enum variant name
* Rust [serde](https://serde.rs/) will be translating this enum between API requests such that specifying a filter in a request would use `shuffle_simd` for the enum `Shuffle_SIMD`

Rust cargo will however complain if we use this enum:

```rust
pub enum Filter {
    /// Byte shuffle
    Shuffle { element_size: usize },
    /// Another byte shuffle, SIMD-accelerated version
    Shuffle_SIMD { element_size: usize },
}
```

complaining with:

```text
116 |     Shuffle_SIMD { element_size: usize },
    |     ^^^^^^^^^^^^ help: convert the identifier to upper camel case: `ShuffleSimd`
```

To keep Rust happy whilst allowing API requests to use `shuffle_simd` we implement the enum as follows:

```rust
pub enum Filter {
    /// Byte shuffle
    Shuffle { element_size: usize },
    /// Another byte shuffle, SIMD-accelerated version
    #[serde(rename = "shuffle_simd")]
    ShuffleSimd { element_size: usize },
}
```

This ensures that wherever the API request is involved it's expecting `shuffle_simd` and not `shufflesimd`.

#### Testing in models.rs

The unit test `test_invalid_filter` in `src/models.rs` must be updated.

Requests made to the Reductionist will name a filter, if used, so we're just making sure the validation for requests is in sync with the filters listed in the enum.

### Add the code behind the defiltering - src/filters.rs, src/filters/shuffle_simd.rs

First we update `src/filters.rs`.

I'm going to mention what you probably already know when it comes to Rust and `cargo clippy`, but it's nice to reiterate.

There's a [match](https://doc.rust-lang.org/rust-by-example/flow_control/match.html) pattern that determines the function used for each filter listed in the `Filter` enum we updated earlier in `src/models.rs`.

Rust is not going to let our code compile unless all match branches are covered, Rust makes it impossible to leave this as a "loose end".

The branch added to this match references a function external to this `src/filters.rs` hence at the top of this file we declare the function:

```rust
pub mod shuffle_simd;
```

This then allows us to add the implementation to a new file `src/filters/shuffle_simd.rs`.

We can start by copying `src/filters/shuffle.rs` to `src/filters/shuffle_simd.rs` so tests are easily adapted since this is just another implementation of a byte-shuffle deshuffling filter.

The [byteshuffle documentation](https://docs.rs/byteshuffle/latest/byteshuffle/) provides a quick start which is enough information to implement a defiltering function.

You might well wonder whether you've implemented this correctly, well we're going to test it!

#### Testing in filters.rs, filters/shuffle_simd.rs

There's already a unit test in `src/filters.rs` that uses code in the corresponding `filters/<filter>.rs` file to filter some data so that the reverse filtering can be tested for a match.

We add a test that follows suit for the new filter.

In `src/filters/shuffle_simd.rs` we have more comprehensive testing as here lies the actual implementation.

### Benchmarking - benches/shuffle.rs

Benchmarking is optional, `benches/shuffle.rs` has been updated as we have the opportunity to benchmark this new byte-shuffle filter addition against the original.

## Compiling the code

You should now be able to compile the code with:

```shell
cargo build
```

What I haven't mentioned in detail are the [use](https://doc.rust-lang.org/rust-by-example/mod/use.html) declarations needed to pull in relevant code dependencies from the `byteshuffle` crate.

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

Whenever you add a new `Filter` variant (such as `ShuffleSimd`), you must also update the public API documentation and any associated schema definitions.

In particular, ensure that the `filter.id` field in `docs/api.md` (and any relevant schema) lists the new algorithm identifier (for example, `shuffle_simd`) alongside the existing values (such as `shuffle`), so that the documented API matches the behavior implemented in `src/models.rs`.

Keeping these documents in sync avoids confusing clients and prevents validation errors when they legitimately use newly supported filters.

## Completing the addition of a new filter

All changes to the Reductionist must be made in a feature branch and merged into the default `main` branch using a pull request.

It will not be possible to merge the changes unless all tests pass within the workflow.

## Concluding whether SIMD-acceleration improves performance

lotRunning benchmarks outputs a let of information.

Each file under the `benches` directory in the root of the repository creates a separate executable.

We can reduce the scope of what the benchmarks execute with:

```shell
cargo bench --bench shuffle
```

to run just the benchmarks in `shuffle.rs`.

Benchmarks store performance data each run so we can look for performance improvement or degradation.

To avoid going too deep into analysis of the output the following shows just the test durations so we can compare the original byte `shuffle` to the newer SIMD-accelerated byte shuffle `shuffle_simd`:

```text
deshuffle(65536, 2)     time:   [1.4776 ms 1.5060 ms 1.5368 ms]

deshuffle_simd(65536, 2)
                        time:   [806.78 µs 810.34 µs 814.32 µs]

deshuffle(65536, 4)     time:   [973.38 µs 1.0116 ms 1.0542 ms]

deshuffle_simd(65536, 4)
                        time:   [764.10 µs 782.75 µs 806.10 µs]

deshuffle(65536, 8)     time:   [852.72 µs 878.92 µs 907.06 µs]

deshuffle_simd(65536, 8)
                        time:   [598.39 µs 608.91 µs 621.69 µs]

deshuffle(262144, 2)    time:   [5.9936 ms 6.1219 ms 6.2597 ms]

deshuffle_simd(262144, 2)
                        time:   [3.3238 ms 3.3726 ms 3.4259 ms]

deshuffle(262144, 4)    time:   [3.8684 ms 3.9543 ms 4.0496 ms]

deshuffle_simd(262144, 4)
                        time:   [3.1684 ms 3.2070 ms 3.2468 ms]

deshuffle(262144, 8)    time:   [3.5358 ms 3.6156 ms 3.7070 ms]

deshuffle_simd(262144, 8)
                        time:   [2.4563 ms 2.4917 ms 2.5317 ms]

deshuffle(1048576, 2)   time:   [22.125 ms 22.310 ms 22.510 ms]

deshuffle_simd(1048576, 2)
                        time:   [14.332 ms 14.757 ms 15.232 ms]

deshuffle(1048576, 4)   time:   [14.273 ms 14.361 ms 14.458 ms]

deshuffle_simd(1048576, 4)
                        time:   [12.268 ms 12.584 ms 12.958 ms]

deshuffle(1048576, 8)   time:   [13.333 ms 13.456 ms 13.588 ms]

deshuffle_simd(1048576, 8)
                        time:   [9.6107 ms 9.7639 ms 9.9573 ms]

deshuffle(4194304, 2)   time:   [132.12 ms 134.39 ms 136.89 ms]

deshuffle_simd(4194304, 2)
                        time:   [84.205 ms 85.217 ms 86.296 ms]

deshuffle(4194304, 4)   time:   [96.291 ms 97.229 ms 98.244 ms]

deshuffle_simd(4194304, 4)
                        time:   [76.597 ms 77.399 ms 78.373 ms]

deshuffle(4194304, 8)   time:   [91.219 ms 92.672 ms 94.329 ms]

deshuffle_simd(4194304, 8)
                        time:   [70.878 ms 72.563 ms 74.524 ms]
```