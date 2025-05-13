import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";

export default defineConfig({
    base: "",
    plugins: [wasm()],
    worker: {
        format: "es",
        plugins: () => [wasm()],
    },
    build: {
        assetsDir: "assets",
    },
});
