# List available recipes
default:
    @just --list

openapi_nexus := env_var_or_default("OPENAPI_NEXUS", "openapi-nexus")

# ---------- Rust ----------

# Build the Rust workspace
build:
    cargo build --workspace

# Check Rust formatting
fmt-check:
    cargo fmt --all -- --check

# Apply Rust formatting
fmt:
    cargo fmt --all

# Run clippy
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Run Rust tests
test:
    cargo test --workspace

# Run the backend locally
dev-server:
    cargo run -p token-toxication-server --bin token-toxication-server

# Generate the OpenAPI JSON document from utoipa
openapi-generate:
    cargo run -p token-toxication-server --bin token-toxication-server -- generate-openapi --output openapi/token-toxication.openapi.json

# Generate the admin TypeScript SDK from the OpenAPI document
sdk-generate: openapi-generate
    rm -rf apps/admin/src/generated/token-toxication
    {{openapi_nexus}} generate \
        --input openapi/token-toxication.openapi.json \
        --generators typescript-fetch \
        --output apps/admin/src/generated/token-toxication \
        --config openapi-nexus.toml \
        --generator-config typescript-fetch.package_name=@token-toxication/admin-api
    cd apps/admin && vp format src/generated/token-toxication

# ---------- Frontend ----------

# Install admin UI dependencies with Vite+
ui-install:
    cd apps/admin && vp install

# Run Vite+ frontend checks
ui-check:
    cd apps/admin && vp check

# Run focused frontend tests
ui-test:
    cd apps/admin && vp test --run

# Build admin UI
ui-build:
    cd apps/admin && vp build

# Serve admin UI in development mode
ui-dev:
    cd apps/admin && vp dev

# ---------- CI ----------

# Run the full local CI pipeline
ci: fmt-check clippy test sdk-generate ui-check ui-test ui-build
