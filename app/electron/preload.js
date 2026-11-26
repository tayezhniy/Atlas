const { contextBridge, ipcRenderer } = require('electron')

// Безопасное API для рендерера. Типы — в app/ui/src/types.ts.
const invoke = (channel) => (arg) => ipcRenderer.invoke(`atlas:${channel}`, arg)

contextBridge.exposeInMainWorld('atlasAPI', {
  ping: invoke('ping'),
  chooseProject: invoke('choose-project'),
  openProject: invoke('open-project'),
  closeProject: invoke('close-project'),
  indexProject: invoke('index-project'),
  watch: invoke('watch'),
  doctor: invoke('doctor'),
  overview: invoke('overview'),
  status: invoke('status'),
  map: invoke('map'),
  appInfo: invoke('app-info'),
  search: invoke('search'),
  outline: invoke('outline'),
  read: invoke('read'),
  neighbors: invoke('neighbors'),
  mcpConfig: invoke('mcp-config'),
  report: invoke('report'),
  settings: {
    all: invoke('settings-all'),
    set: (projectPath, key, value) =>
      ipcRenderer.invoke('atlas:settings-set', { projectPath, key, value }),
  },
})
