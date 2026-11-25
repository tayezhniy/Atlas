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

  overview: (projectPath, path) =>
    ipcRenderer.invoke('atlas:overview', { projectPath, path }),

  outline: ({ projectPath, path: filePath }) =>
    ipcRenderer.invoke('atlas:outline', { projectPath, path: filePath }),

  read: ({ projectPath, path: filePath, start_line, end_line, mode }) =>
    ipcRenderer.invoke('atlas:read', {
      projectPath,
      path: filePath,
      start_line,
      end_line,
      mode: mode || 'full',
    }),

  neighbors: ({ projectPath, target }) =>
    ipcRenderer.invoke('atlas:neighbors', { projectPath, target }),

  contextFor: ({ projectPath, symbol, path, max_tokens }) =>
    ipcRenderer.invoke('atlas:context-for', { projectPath, symbol, path, max_tokens }),

  status: (projectPath) =>
    ipcRenderer.invoke('atlas:status', projectPath),

  tree: (projectPath, path) =>
    ipcRenderer.invoke('atlas:tree', { projectPath, path }),

  callLog: (projectPath, options) =>
    ipcRenderer.invoke('atlas:call-log', { projectPath, options }),

  settings: {
    get: (projectPath, key) =>
      ipcRenderer.invoke('atlas:settings-get', { projectPath, key }),
    set: (projectPath, key, value) =>
      ipcRenderer.invoke('atlas:settings-set', { projectPath, key, value }),
    all: (projectPath) =>
      ipcRenderer.invoke('atlas:settings-all', projectPath),
  },

  report: (projectPath, options) =>
    ipcRenderer.invoke('atlas:report', { projectPath, options }),

  closeProject: (projectPath) =>
    ipcRenderer.invoke('atlas:close-project', projectPath),
})

// Типизация для TypeScript (используется в ui/src)
const atlasAPI = window.atlasAPI
