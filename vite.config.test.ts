import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { type Plugin, defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

// Plugin to serve index-test.html at root URL
// This ensures the test harness frontend loads instead of the regular app
function serveTestHtmlPlugin(): Plugin {
  return {
    name: "serve-test-html",
    configureServer(server) {
      server.middlewares.use((req, _res, next) => {
        // Rewrite root URL to index-test.html
        if (req.url === "/" || req.url === "/index.html") {
          req.url = "/index-test.html";
        }
        next();
      });
    },
  };
}

// Vite config for test harness build
// Uses a separate entry point and output directory
export default defineConfig(async () => ({
  plugins: [serveTestHtmlPlugin(), react(), tailwindcss()],

  // Different entry point for test harness
  // Output must be index.html for Tauri to find it
  build: {
    outDir: "dist-test",
    rollupOptions: {
      input: {
        index: "index-test.html",
      },
    },
  },

  // Define test mode flag
  define: {
    __TEST_MODE__: JSON.stringify(true),
  },

  clearScreen: false,
  server: {
    port: 1421, // Different port for test harness
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1422,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  resolve: {
    alias: {
      "@tabler/icons-react": "@tabler/icons-react/dist/esm/icons/index.mjs",
    },
  },
}));
