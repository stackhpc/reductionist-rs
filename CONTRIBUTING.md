# Contributing

## Pre-commit hook

A pre-commit hook is provided in `tools/pre-commit` that runs formatting,
clippy, and unit tests. After cloning this repository, copy it to
`.git/hooks/pre-commit`.

## Continuous Integration (CI)

GitHub Actions is used for CI for pull requests.
It checks that the package builds, and passes checks and unit tests.
