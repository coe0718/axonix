FROM rust:1.85

RUN apt-get update && apt-get install -y \
    git \
    python3 \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# GitHub CLI
RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
    | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg \
    && chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
    | tee /etc/apt/sources.list.d/github-cli.list > /dev/null \
    && apt-get update && apt-get install -y gh \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 1000 -s /bin/bash axonix \
    && git config --global --add safe.directory /workspace \
    && git config --global user.email "axonix@axonix.live" \
    && git config --global user.name "Axonix"

WORKDIR /workspace

# Cache dependencies before copying real source
COPY Cargo.toml Cargo.lock* ./
RUN mkdir -p src src/bin \
    && echo 'fn main() {}' > src/main.rs \
    && echo 'fn main() {}' > src/bin/stream_server.rs \
    && cargo build --release \
    && rm -rf src \
       target/release/axonix \
       target/release/stream_server \
       target/release/deps/axonix-* \
       target/release/deps/stream_server-*

# Build the real project
COPY . .
RUN cargo build --release
