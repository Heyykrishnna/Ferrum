FROM rust:1.100.0-trixie AS verification-base

WORKDIR /workspace

RUN rustup component add clippy rustfmt \
    && useradd --create-home --uid 10001 ferrum-check \
    && install -d -o ferrum-check -g ferrum-check /workspace/target

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY apps/ apps/
COPY crates/ crates/
COPY migrations/ migrations/

ENV CARGO_HOME=/home/ferrum-check/.cargo

USER ferrum-check
RUN cargo fetch --locked --target x86_64-unknown-linux-gnu

ENV CARGO_NET_OFFLINE=true

FROM verification-base AS language-toolchains

USER root
RUN apt-get update \
    && apt-get install -y --no-install-recommends python3 gcc-14 g++-14 nodejs openjdk-21-jdk-headless rustc \
    && rm -rf /var/lib/apt/lists/*
USER ferrum-check
CMD ["sh", "-ec", "cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked && cargo test --locked -p ferrum-sandbox --test toolchain_plans -- --ignored"]

FROM verification-base AS sandbox-checks

CMD ["sh", "-ec", "cargo fmt --all --check && cargo clippy --locked -p ferrum-domain -p ferrum-sandbox --all-targets -- -D warnings && cargo test --locked -p ferrum-domain -p ferrum-sandbox"]
