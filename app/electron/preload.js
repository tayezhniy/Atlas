const { contextBridge, ipcRenderer } = require('electron')

// ---------------------------------------------------------------------------
// Preload: безопасное API для рендерера (замена Tauri invoke).
// Все вызовы идут через IPC в main process, который проксирует их в
// Rust-MCP-сервер (stdio JSON-RPC) или в CLI-бинарник atlas-cli.
// ---------------------------------------------------------------------------

contextBridge.exposeInMainWorld('atlasAPI', {
  ping: () => ipcRenderer.invoke('atlas:ping'),

  openProject: (projectPath) =>
    ipcRenderer.invoke('atlas:open-project', projectPath),

  indexProject: (projectPath) =>
    ipcRenderer.invoke('atlas:index-project', projectPath),

  doctor: (projectPath) =>
    ipcRenderer.invoke('atlas:doctor', projectPath),

  search: ({ projectPath, query, limit }) =>
    ipcRenderer.invoke('atlas:search', { projectPath, query, limit }),

  overview: (projectPath) =>
    ipcRenderer.invoke('atlas:overview', { projectPath }),

  outline: ({ projectPath, path: filePath }) =>
    ipcRenderer.invoke('atlas:outline', { projectPath, path: filePath }),

  read: ({ projectPath, path: filePath, start_line, end_line }) =>
    ipcRenderer.invoke('atlas:read', {
      projectPath,
      path: filePath,
      start_line,
      end_line,
    }),

  neighbors: ({ projectPath, target }) =>
    ipcRenderer.invoke('atlas:neighbors', { projectPath, target }),

  closeProject: (projectPath) =>
    ipcRenderer.invoke('atlas:close-project', projectPath),
})

// Типизация для TypeScript (используется в ui/src)
const atlasAPI = window.atlasAPI
