.PHONY: build
build:
	@docker buildx build -t reductionist .

.PHONY: docs
docs:
	@docker buildx build --build-arg PROFILE=dev --target builder -t reductionist-test .
	@docker run --rm -e RUSTDOCFLAGS="-D warnings" reductionist-test cargo doc --no-deps

.PHONY: lint
lint:
	@docker buildx build --build-arg PROFILE=dev --target builder -t reductionist-test .
	@docker run --rm reductionist-test cargo check --color always
	@docker run --rm reductionist-test bash -c 'rustup component add rustfmt && cargo fmt -- --color always --check'
	@docker run --rm reductionist-test bash -c 'rustup component add clippy && cargo clippy --all-targets -- -D warnings'

.PHONY: test
test:
	@docker buildx build --build-arg PROFILE=dev --target builder -t reductionist-test .
	@docker run --rm reductionist-test cargo test --color always

.PHONY: run
run:
	@docker run -it --detach --rm --net=host --name reductionist reductionist

.PHONY: run-with-cache
run-with-cache:
	@docker run -it --detach --rm --net=host --name reductionist reductionist reductionist --use-chunk-cache --chunk-cache-path ./

.PHONY: stop
stop:
	@docker stop reductionist
