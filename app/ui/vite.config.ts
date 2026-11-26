import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  // окно Electron открывает файл по file://, нужны относительные пути
  base: './',
  build: {
    outDir: '../electron/gen/dist',
    emptyOutDir: true,
  },
  css: {
    postcss: './postcss.config.js',
  },
})
