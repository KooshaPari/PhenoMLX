# HFScope development tasks
SHELL := /bin/zsh
BIN  := bin/hfscope
PKG  := ./cmd/hfscope ./...

.PHONY: all build run test tidy fmt vet generate watch clean

all: generate build

generate:
	templ generate

build: generate
	mkdir -p bin
	go build -trimpath -o $(BIN) ./cmd/hfscope

run:
	go run ./cmd/hfscope

watch:
	air

tidy:
	go mod tidy

fmt:
	go fmt $(PKG)

vet:
	go vet $(PKG)

test:
	go test ./...

clean:
	rm -rf bin
	rm -f $(shell find internal/server -name '*_templ.go')