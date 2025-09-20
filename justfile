# Set environment variable of rust backtrace to full
export RUST_BACKTRACE:= "full"


# Default recipe which list all available recipes
default:
  just --list --justfile {{justfile()}}

# Clean target directory
clean:
    cargo clean

# Run cargo build
build *args="--all-features":
    cargo build --workspace {{args}}

# Check whether rust code is properly formatted or not (nightly only)
fmt:
    #!/usr/bin/env bash
    if [[ "$(rustc --version)" == *nightly* ]]; then
        echo "Checking if rust is properly formatted"
        cargo fmt -- --check
    fi

# Run clippy to catch common mistakes and improve code (nightly only)
clippy *args="--all-features":
    #!/usr/bin/env bash
    if [[ "$(rustc --version)" == *nightly* ]]; then
        echo "Checking common mistakes in code"
        cargo clippy --workspace {{args}} -- -D warnings
    fi

# Run tests
test *args="--all-features":
    cargo test --workspace {{args}}

# Generate documentation
doc *args="--all-features":
    cargo doc --workspace --no-deps {{args}}

# Run rustdoc with docsrs configuration
rustdoc:
    cargo rustdoc --all-features -- --cfg docsrs

# Local development tasks i.e fmt, build, clippy, doc and test
local: fmt build clippy doc test

# Full build including clean
full: clean local
