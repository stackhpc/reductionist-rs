.PHONY: build
build:
	@docker build -t s3-active-storage .

.PHONY: run
run:
	@docker run -it --rm --name s3-active-storage s3-active-storage
