.PHONY: build
build:
	@docker buildx build -t s3-active-storage .

.PHONY: test
test:
	@docker buildx build --build-arg PROFILE=dev --target builder -t s3-active-storage-test .
	@docker run --rm s3-active-storage-test cargo check --color always
	@docker run --rm s3-active-storage-test cargo test --color always

.PHONY: run
run:
	@docker run -it --detach --rm --net=host --name s3-active-storage s3-active-storage

.PHONY: stop
stop:
	@docker stop s3-active-storage
