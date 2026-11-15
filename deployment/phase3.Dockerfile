FROM rust:1.100.0-trixie

WORKDIR /workspace

RUN rustup component add clippy rustfmt \
    && useradd --create-home --uid 10001 ferrum-check \
    && install -d -o ferrum-check -g ferrum-check /workspace/target

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY apps/api/ apps/api/
COPY crates/ crates/
COPY migrations/ migrations/

ENV CARGO_HOME=/home/ferrum-check/.cargo

USER ferrum-check
RUN cargo fetch --locked --target x86_64-unknown-linux-gnu

ENV CARGO_NET_OFFLINE=true

CMD ["sh", "-ec", "cargo fmt --all --check && cargo clippy --locked -p ferrum-domain -p ferrum-sandbox --all-targets -- -D warnings && cargo test --locked -p ferrum-domain -p ferrum-sandbox"]
