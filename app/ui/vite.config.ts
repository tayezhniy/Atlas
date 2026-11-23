import { defineConfig } from 'vite'
import tauri from 'tauri-plugin-vite'

export default defineConfig({
  plugins: [tauri()],
  build: {
    outDir: '../src-tauri/gen/dist',
    emptyOutDir: true,
  },
})