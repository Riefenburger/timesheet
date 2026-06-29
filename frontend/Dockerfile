# ===== Stage 1: build the static site =====
FROM node:22-slim AS builder

WORKDIR /app

# Install deps first (cached layer — only re-runs when package files change).
COPY package.json package-lock.json ./
RUN npm ci

# Copy the rest and generate the static site into /app/.output/public
COPY . .
RUN npm run generate

# ===== Stage 2: serve the static files =====
FROM caddy:2-alpine AS runtime

# Copy the built static site into Caddy's web root.
COPY --from=builder /app/.output/public /usr/share/caddy

EXPOSE 80
# caddy:2-alpine's default config already serves /usr/share/caddy on :80