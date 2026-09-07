# ===== Stage 1: build the static site =====
FROM node:22-slim AS builder

WORKDIR /app

# Install deps first (cached layer — only re-runs when package files change).
COPY package.json package-lock.json ./
RUN npm ci

# Copy the rest and generate the static site into /app/.output/public
COPY . .
RUN npm run generate

# ===== Stage 2: serve the static files + proxy /api =====
FROM caddy:2-alpine AS runtime

# Custom Caddy config: serve the SPA and proxy /api to the backend.
COPY Caddyfile /etc/caddy/Caddyfile

# Copy the built static site into Caddy's web root.
COPY --from=builder /app/.output/public /usr/share/caddy

EXPOSE 80