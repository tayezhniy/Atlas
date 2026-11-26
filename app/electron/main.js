const { app, BrowserWindow, ipcMain, dialog } = require('electron')
const path = require('node:path')
const fs = require('node:fs')
const { spawn } = require('node:child_process')

// ---------------------------------------------------------------------------
// Оболочка Atlas. Main process держит по процессу `atlas-cli mcp` на проект и
// берёт данные для интерфейса методом `atlas/ui` (JSON-RPC по stdio).
// ---------------------------------------------------------------------------

let mainWindow = null
let nextId = 1
// projectPath -> { proc, pending: Map<id, {resolve, reject}> }
const servers = new Map()
const watchers = new Map()

function binaryPath() {
  if (process.env.ATLAS_BIN) return process.env.ATLAS_BIN
  const name = process.platform === 'win32' ? 'atlas-cli.exe' : 'atlas-cli'
  if (app.isPackaged) return path.join(process.resourcesPath, name)
  const built = ['release', 'debug']
    .map((profile) => path.join(__dirname, '..', '..', 'target', profile, name))
    .filter((p) => fs.existsSync(p))
    // самый свежий, чтобы старая release-сборка не перекрывала новую debug
    .sort((a, b) => fs.statSync(b).mtimeMs - fs.statSync(a).mtimeMs)
  return built[0] ?? name
}

function startServer(projectPath) {
  const proc = spawn(binaryPath(), ['mcp', '--project', projectPath], {
    stdio: ['pipe', 'pipe', 'pipe'],
  })
  const server = { proc, pending: new Map() }
  let buffer = ''

  const fail = (error) => {
    for (const cb of server.pending.values()) cb.reject(error)
    server.pending.clear()
    if (servers.get(projectPath) === server) servers.delete(projectPath)
  }

  proc.stdout.setEncoding('utf8')
  proc.stdout.on('data', (chunk) => {
    buffer += chunk
    let idx
    while ((idx = buffer.indexOf('\n')) !== -1) {
      const line = buffer.slice(0, idx)
      buffer = buffer.slice(idx + 1)
      let msg
      try {
        msg = JSON.parse(line)
      } catch {
        continue // не JSON — это логи
      }
      const cb = server.pending.get(msg.id)
      if (!cb) continue
      server.pending.delete(msg.id)
      if (msg.error) cb.reject(new Error(msg.error.message))
      else cb.resolve(msg.result)
    }
  })
  proc.stderr.on('data', (chunk) => console.debug('[atlas]', chunk.toString().trim()))
  proc.stdin.on('error', fail)
  proc.on('error', fail)
  proc.on('exit', () => fail(new Error('Процесс atlas-cli завершился')))

  servers.set(projectPath, server)
  return server
}

function request(projectPath, method, params) {
  const server = servers.get(projectPath) ?? startServer(projectPath)
  const id = nextId++
  return new Promise((resolve, reject) => {
    server.pending.set(id, { resolve, reject })
    server.proc.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n')
  })
}

const ui = (projectPath, method, args = {}) => request(projectPath, 'atlas/ui', { method, args })

function runCli(args) {
  return new Promise((resolve, reject) => {
    const proc = spawn(binaryPath(), args, { stdio: ['ignore', 'pipe', 'pipe'] })
    let out = ''
    let err = ''
    proc.stdout.setEncoding('utf8')
    proc.stdout.on('data', (c) => (out += c))
    proc.stderr.on('data', (c) => (err += c))
    proc.on('error', reject)
    proc.on('close', (code) => {
      if (code === 0) resolve(out.trim())
      else reject(new Error(err.trim() || out.trim() || `код ${code}`))
    })
  })
}

function stopProject(projectPath) {
  servers.get(projectPath)?.proc.kill()
  servers.delete(projectPath)
  watchers.get(projectPath)?.kill()
  watchers.delete(projectPath)
}

function stopAll() {
  for (const projectPath of new Set([...servers.keys(), ...watchers.keys()])) stopProject(projectPath)
}

// ---------------------------------------------------------------------------
// IPC (renderer -> main)
// ---------------------------------------------------------------------------

ipcMain.handle('atlas:ping', () => 'pong')

ipcMain.handle('atlas:choose-project', async () => {
  const result = await dialog.showOpenDialog(mainWindow, { properties: ['openDirectory'] })
  return result.canceled ? null : result.filePaths[0]
})

ipcMain.handle('atlas:open-project', async (_e, projectPath) => {
  if (!fs.statSync(projectPath).isDirectory()) throw new Error('Это не каталог')
  await request(projectPath, 'initialize', { protocolVersion: '2025-03-26', capabilities: {} })
  return { ok: true, projectPath }
})

ipcMain.handle('atlas:close-project', (_e, projectPath) => {
  stopProject(projectPath)
  return { ok: true }
})

ipcMain.handle('atlas:index-project', async (_e, projectPath) => ({
  ok: true,
  output: await runCli(['index', projectPath]),
}))

ipcMain.handle('atlas:watch', (_e, projectPath) => {
  if (!watchers.has(projectPath)) {
    const proc = spawn(binaryPath(), ['watch', '--path', projectPath], { stdio: 'ignore' })
    const forget = () => {
      if (watchers.get(projectPath) === proc) watchers.delete(projectPath)
    }
    proc.on('error', forget)
    proc.on('exit', forget)
    watchers.set(projectPath, proc)
  }
  return { ok: true }
})

ipcMain.handle('atlas:doctor', async (_e, projectPath) => ({
  ok: true,
  output: await runCli(['doctor', '--project', projectPath]),
}))

ipcMain.handle('atlas:overview', (_e, projectPath) => ui(projectPath, 'overview'))
ipcMain.handle('atlas:status', (_e, projectPath) => ui(projectPath, 'status'))
for (const method of ['search', 'outline', 'read', 'neighbors']) {
  ipcMain.handle(`atlas:${method}`, (_e, { projectPath, ...args }) => ui(projectPath, method, args))
}

ipcMain.handle('atlas:mcp-config', (_e, projectPath) =>
  JSON.stringify(
    { mcpServers: { atlas: { command: binaryPath(), args: ['mcp', '--project', projectPath] } } },
    null,
    2,
  ),
)

ipcMain.handle('atlas:settings-set', async (_e, { projectPath, key, value }) => {
  await runCli(['settings', 'set', '--project', projectPath, '--', key, value])
  return { ok: true }
})

ipcMain.handle('atlas:settings-all', async (_e, projectPath) => ({
  ok: true,
  settings: JSON.parse(await runCli(['settings', 'all', '--project', projectPath])),
}))

ipcMain.handle('atlas:report', async (_e, { projectPath, format }) => {
  const out = await runCli(['report', '--project', projectPath, '--format', format])
  return format === 'json' ? JSON.parse(out) : out
})

// ---------------------------------------------------------------------------
// Окно
// ---------------------------------------------------------------------------

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1400,
    height: 900,
    minWidth: 1000,
    minHeight: 700,
    title: 'Atlas — навигатор по проекту',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  })

  if (process.env.VITE_DEV_SERVER_URL) {
    mainWindow.loadURL(process.env.VITE_DEV_SERVER_URL)
  } else {
    mainWindow.loadFile(path.join(__dirname, 'gen/dist/index.html'))
  }

  mainWindow.webContents.setWindowOpenHandler(() => ({ action: 'deny' }))
}

app.whenReady().then(() => {
  createWindow()
  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow()
  })
})

app.on('will-quit', stopAll)
app.on('window-all-closed', () => {
  stopAll()
  if (process.platform !== 'darwin') app.quit()
})
