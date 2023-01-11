.PHONY: build
build:
	@docker buildx build -t s3-active-storage .

.PHONY: test
test:
	@docker buildx build --target builder -t s3-active-storage-test .
	@docker run --rm s3-active-storage-test cargo check --color always
	@docker run --rm s3-active-storage-test cargo test --color always

.PHONY: run
run:
	@docker run -it --rm --net=host --name s3-active-storage s3-active-storage
