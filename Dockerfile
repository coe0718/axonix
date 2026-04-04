FROM rust:1.85

RUN apt-get update && apt-get install -y \
    git \
    python3 \
    curl \
    perl \
    && rm -rf /var/lib/apt/lists/*

# GitHub CLI — download .deb directly via Docker ADD (Go HTTP, no OpenSSL/curl)
ADD https://github.com/cli/cli/releases/download/v2.68.0/gh_2.68.0_linux_amd64.deb /tmp/gh.deb
RUN dpkg -i /tmp/gh.deb && rm /tmp/gh.deb

RUN useradd -m -u 1000 -s /bin/bash axonix \
    && git config --global --add safe.directory /workspace \
    && git config --global user.email "axonix@axonix.live" \
    && git config --global user.name "Axonix"

WORKDIR /workspace

ENV CARGO_INCREMENTAL=0

# Cache dependencies before copying real source
COPY Cargo.toml Cargo.lock* ./
COPY vendor/ ./vendor/
RUN mkdir -p src src/bin \
    && echo 'fn main() {}' > src/main.rs \
    && echo 'fn main() {}' > src/bin/stream_server.rs \
    && cargo build \
    && rm -rf src \
       target/debug/axonix \
       target/debug/stream_server \
       target/debug/deps/axonix-* \
       target/debug/deps/stream_server-*

# Build the real project
COPY . .
RUN cargo build
