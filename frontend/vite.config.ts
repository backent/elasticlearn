import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// In `docker compose`, the frontend container reaches the backend by service
// name (`http://backend:8080`). For local `npm run dev` outside Docker, the
// fallback `http://localhost:8080` is correct.
const backend = process.env.BACKEND_URL ?? "http://localhost:8080";

export default defineConfig({
  plugins: [react()],
  server: {
    host: "0.0.0.0",
    port: 5173,
    proxy: {
      "/api": {
        target: backend,
        changeOrigin: false,
      },
    },
  },
});
