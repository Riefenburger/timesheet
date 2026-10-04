import tailwindcss from "@tailwindcss/vite";

export default defineNuxtConfig({
  compatibilityDate: "2025-07-15",
  devtools: { enabled: true },
  ssr: false,
  css: ["~/assets/css/main.css"],
  app: {
    head: {
      // Declared explicitly: the default favicon.ico is gone, replaced by the
      // studio's dancer mark.
      link: [
        { rel: "icon", type: "image/png", href: "/favicon.png" },
        { rel: "apple-touch-icon", href: "/apple-touch-icon.png" },
      ],
    },
  },
  vite: {
    plugins: [tailwindcss()],
  },
  devServer: {
    port: 3001,
  },
  routeRules: {
    "/api/**": { proxy: "http://localhost:3000/**" },
  },
});