# ── Stage 1: build vargc from source ─────────────────────────────
FROM rust:slim-bookworm AS vargc-builder
WORKDIR /workspace
COPY varg-compiler/ ./varg-compiler/
RUN cd varg-compiler && cargo build --release -p vargc

# ── Stage 2: publish .NET Blazor app ─────────────────────────────
FROM mcr.microsoft.com/dotnet/sdk:10.0 AS dotnet-builder
WORKDIR /app
COPY varg-web-ide/VargWebIde/ .
RUN dotnet publish -c Release -o /publish

# ── Stage 3: runtime ──────────────────────────────────────────────
FROM mcr.microsoft.com/dotnet/aspnet:10.0
WORKDIR /app

# Rust toolchain + rustfmt (vargc runs rustfmt on generated code)
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        curl ca-certificates build-essential \
        pkg-config libssl-dev && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && \
    /root/.cargo/bin/rustup component add rustfmt && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

ENV PATH="/root/.cargo/bin:${PATH}"

# Published Blazor app
COPY --from=dotnet-builder /publish .

# vargc binary
COPY --from=vargc-builder /workspace/varg-compiler/target/release/vargc /usr/local/bin/vargc

# Varg runtime crates — vargc resolves path deps relative to cwd,
# so we place them here and symlink from each temp build dir.
COPY --from=vargc-builder /workspace/varg-compiler/crates/varg-os-types /opt/varg/crates/varg-os-types
COPY --from=vargc-builder /workspace/varg-compiler/crates/varg-runtime   /opt/varg/crates/varg-runtime

ENV Varg__VargcPath=vargc
ENV Varg__BuildCache=/tmp/varg-cache
ENV Varg__CratesDir=/opt/varg/crates
ENV ASPNETCORE_URLS=http://+:8080
ENV DOTNET_RUNNING_IN_CONTAINER=true

EXPOSE 8080

ENTRYPOINT ["dotnet", "VargWebIde.dll"]
