.PHONY: build
build:
	@docker buildx build -t reductionist .

.PHONY: test
test:
	@docker buildx build --build-arg PROFILE=dev --target builder -t reductionist-test .
	@docker run --rm reductionist-test cargo check --color always
	@docker run --rm reductionist-test cargo test --color always
	@docker run --rm reductionist-test cargo bench --color always

.PHONY: run
run:
	@docker run -it --detach --rm --net=host --name reductionist reductionist

.PHONY: stop
stop:
	@docker stop reductionist
