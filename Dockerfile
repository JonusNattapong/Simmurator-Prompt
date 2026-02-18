# Stage 1: Build Frontend
FROM node:20-alpine AS frontend-builder
WORKDIR /app
COPY package*.json ./
RUN npm install
COPY . .
RUN npm run build

# Stage 2: Build Backend (Rust)
FROM rust:1-slim AS backend-builder
WORKDIR /app
COPY server/Cargo.toml server/Cargo.lock* ./server/
COPY server/src ./server/src
WORKDIR /app/server
RUN cargo build --release

# Stage 3: Runtime
FROM debian:bookworm-slim
WORKDIR /app
# Copy frontend assets
COPY --from=frontend-builder /app/dist ./dist
# Copy backend binary
COPY --from=backend-builder /app/server/target/release/simmurator-server ./server-bin

EXPOSE 8080
# Start the server (ensure paths for static files match the binary location)
CMD ["./server-bin"]
