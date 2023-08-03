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

.PHONY: test
test:
	@docker buildx build --build-arg PROFILE=dev --target builder -t reductionist-test .
	@docker run --rm reductionist-test cargo test --color always

.PHONY: run
run:
	@docker run -it --detach --rm --net=host --name reductionist reductionist

.PHONY: stop
stop:
	@docker stop reductionist
