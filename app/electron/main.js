const { app, BrowserWindow, ipcMain } = require('electron')
const path = require('node:path')
const { spawn } = require('node:child_process')
const fs = require('node:fs')

// ---------------------------------------------------------------------------
// Электрон-оболочка Atlas (замена Tauri).
// Main process порождает Rust-бинарник `atlas-cli mcp` и проксирует JSON-RPC
// через stdio. Рендерер общается с main через IPC (preload -> ipcRenderer).
// ---------------------------------------------------------------------------

let mainWindow = null
// projectPath -> { proc, pendingRequests: Map<id, {resolve, reject}>, buffer }
const mcpProcesses = new Map()

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

  // В разработке загружаем с Vite dev-сервера; в сборке — из файла.
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

app.on('window-all-closed', () => {
  // Завершаем все MCP-процессы при закрытии окон
  for (const { proc } of mcpProcesses.values()) {
    try { proc.kill('SIGTERM') } catch (_) {}
  }
  mcpProcesses.clear()
  if (process.platform !== 'darwin') app.quit()
})

// ---------------------------------------------------------------------------
// Утилиты
// ---------------------------------------------------------------------------

function getMcpBinary() {
  // В продакшн-сборке бинарник лежит рядом с resources/app.asar.unpacked
  // В dev — в target/release или target/debug
  const isDev = process.env.VITE_DEV_SERVER_URL !== undefined
  const baseDir = isDev 
    ? path.join(__dirname, '..', '..') 
    : path.join(process.resourcesPath, 'app.asar.unpacked', '..', '..')
  
  const candidates = [
    // Release binary (production)
    path.join(baseDir, 'target', 'release', 'atlas-cli'),
    // Debug binary (development)
    path.join(baseDir, 'target', 'debug', 'atlas-cli'),
    // Fallback to PATH
    'atlas-cli',
  ]
  
  // Return first existing binary or the first candidate as fallback
  for (const candidate of candidates) {
    if (candidate !== 'atlas-cli' && fs.existsSync(candidate)) {
      return candidate
    }
  }
  return candidates[0]
}

function parseMcpResult(result) {
  // MCP tools/call returns { content: [{ type: 'text', text: '...' }], isError: false }
  // We need to extract the text content
  if (!result || !result.content || !Array.isArray(result.content)) {
    return result
  }
  
  const textContent = result.content
    .filter(c => c.type === 'text')
    .map(c => c.text)
    .join('\n')
  
  // Try to parse as JSON for structured responses
  try {
    return JSON.parse(textContent)
  } catch {
    // Return raw text if not JSON
    return textContent
  }
}

// ---------------------------------------------------------------------------
// MCP-клиент поверх stdio
// ---------------------------------------------------------------------------

function spawnMcp(projectPath) {
  const bin = getMcpBinary()
  const proc = spawn(bin, ['mcp', '--project', projectPath], {
    stdio: ['pipe', 'pipe', 'pipe'],
  })

  let buffer = ''
  const pending = new Map()

  proc.stdout.on('data', (chunk) => {
    buffer += chunk.toString()
    let idx
    while ((idx = buffer.indexOf('\n')) !== -1) {
      const line = buffer.slice(0, idx)
      buffer = buffer.slice(idx + 1)
      if (!line.trim()) continue
      try {
        const msg = JSON.parse(line)
        if (msg.jsonrpc === '2.0' && msg.id !== undefined) {
          const cb = pending.get(msg.id)
          if (cb) {
            pending.delete(msg.id)
            if (msg.error) cb.reject(new Error(msg.error.message || 'MCP error'))
            else cb.resolve(msg.result)
          }
        }
      } catch (_) {
        // игнорируем не-JSON строки (логи)
      }
    }
  })

  proc.stderr.on('data', (chunk) => {
    // логи MCP-сервера в консоль main process
    console.debug(`[mcp:${projectPath}]`, chunk.toString().trim())
  })

  proc.on('exit', () => {
    for (const [, cb] of pending) {
      cb.reject(new Error('MCP-процесс завершён'))
    }
    pending.clear()
    mcpProcesses.delete(projectPath)
  })

  proc.on('error', (err) => {
    console.error(`[mcp:${projectPath}] spawn error:`, err.message)
    for (const [, cb] of pending) {
      cb.reject(err)
    }
    pending.clear()
    mcpProcesses.delete(projectPath)
  })

  mcpProcesses.set(projectPath, { proc, pending, buffer: '' })

  // Инициализируем протокол MCP (обмен версиями и списком инструментов)
  mcpRequest(projectPath, 'initialize', {
    protocolVersion: '2025-03-26',
    capabilities: {},
  }).catch(() => {})

  return { proc, pending }
}

function getMcp(projectPath) {
  let entry = mcpProcesses.get(projectPath)
  if (!entry) {
    entry = spawnMcp(projectPath)
  }
  return entry
}

function mcpRequest(projectPath, method, params) {
  const entry = getMcp(projectPath)
  const id = Date.now() + Math.random()
  return new Promise((resolve, reject) => {
    entry.pending.set(id, { resolve, reject })
    const message = JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n'
    try {
      entry.proc.stdin.write(message)
    } catch (err) {
      entry.pending.delete(id)
      reject(err)
    }
  })
}

// ---------------------------------------------------------------------------
// Одноразовые команды CLI (index, doctor, run)
// ---------------------------------------------------------------------------

function runCli(args, cwd) {
  return new Promise((resolve, reject) => {
    const bin = getMcpBinary()
    const proc = spawn(bin, args, { stdio: ['pipe', 'pipe', 'pipe'], cwd })
    let out = ''
    let err = ''
    proc.stdout.on('data', (c) => (out += c.toString()))
    proc.stderr.on('data', (c) => (err += c.toString()))
    proc.on('close', (code) => {
      if (code === 0) resolve(out.trim())
      else reject(new Error(err.trim() || `код ${code}`))
    })
    proc.on('error', reject)
  })
}

// ---------------------------------------------------------------------------
// IPC-хендлеры (renderer -> main)
// ---------------------------------------------------------------------------

ipcMain.handle('atlas:ping', async () => 'pong')

ipcMain.handle('atlas:open-project', async (_event, projectPath) => {
  // Просто убеждаемся, что MCP-процесс для проекта запущен
  await getMcp(projectPath)
  return { ok: true, projectPath }
})

ipcMain.handle('atlas:index-project', async (_event, projectPath) => {
  const out = await runCli(['index', projectPath], projectPath)
  return { ok: true, output: out }
})

ipcMain.handle('atlas:doctor', async (_event, projectPath) => {
  try {
    const result = await mcpRequest(projectPath, 'tools/call', {
      name: 'status',
      arguments: {},
    })
    return { ok: true, result: parseMcpResult(result) }
  } catch (err) {
    const out = await runCli(['doctor', '--project', projectPath], projectPath)
    return { ok: true, output: out }
  }
})

ipcMain.handle('atlas:search', async (_event, { projectPath, query, limit }) => {
  const result = await mcpRequest(projectPath, 'tools/call', {
    name: 'search',
    arguments: { query, limit: limit ?? 20 },
  })
  return parseMcpResult(result)
})

ipcMain.handle('atlas:overview', async (_event, { projectPath, path }) => {
  const result = await mcpRequest(projectPath, 'tools/call', {
    name: 'overview',
    arguments: { path },
  })
  return parseMcpResult(result)
})

ipcMain.handle('atlas:outline', async (_event, { projectPath, path: filePath }) => {
  const result = await mcpRequest(projectPath, 'tools/call', {
    name: 'outline',
    arguments: { path: filePath },
  })
  return parseMcpResult(result)
})

ipcMain.handle('atlas:read', async (_event, { projectPath, path: filePath, start_line, end_line, mode }) => {
  const result = await mcpRequest(projectPath, 'tools/call', {
    name: 'read',
    arguments: { path: filePath, start_line, end_line, mode: mode || 'full' },
  })
  return parseMcpResult(result)
})

ipcMain.handle('atlas:neighbors', async (_event, { projectPath, target }) => {
  const result = await mcpRequest(projectPath, 'tools/call', {
    name: 'neighbors',
    arguments: { symbol: target },
  })
  return parseMcpResult(result)
})

ipcMain.handle('atlas:context-for', async (_event, { projectPath, symbol, path, max_tokens }) => {
  const result = await mcpRequest(projectPath, 'tools/call', {
    name: 'context_for',
    arguments: { symbol, path, max_tokens },
  })
  return parseMcpResult(result)
})

ipcMain.handle('atlas:status', async (_event, projectPath) => {
  const result = await mcpRequest(projectPath, 'tools/call', {
    name: 'status',
    arguments: {},
  })
  return parseMcpResult(result)
})

ipcMain.handle('atlas:tree', async (_event, { projectPath, path }) => {
  const result = await mcpRequest(projectPath, 'tools/call', {
    name: 'overview',
    arguments: { path },
  })
  return parseMcpResult(result)
})

ipcMain.handle('atlas:call-log', async (_event, { projectPath, options }) => {
  const result = await mcpRequest(projectPath, 'tools/call', {
    name: 'status',
    arguments: {},
  })
  return parseMcpResult(result)
})

// Settings stored in SQLite via atlas-cli settings commands
ipcMain.handle('atlas:settings-get', async (_event, { projectPath, key }) => {
  try {
    const out = await runCli(['settings', 'get', '--project', projectPath, key], projectPath)
    return { ok: true, value: out }
  } catch (err) {
    return { ok: false, error: err.message }
  }
})

ipcMain.handle('atlas:settings-set', async (_event, { projectPath, key, value }) => {
  try {
    await runCli(['settings', 'set', '--project', projectPath, key, value], projectPath)
    return { ok: true }
  } catch (err) {
    return { ok: false, error: err.message }
  }
})

ipcMain.handle('atlas:settings-all', async (_event, projectPath) => {
  try {
    const out = await runCli(['settings', 'all', '--project', projectPath], projectPath)
    return { ok: true, settings: JSON.parse(out) }
  } catch (err) {
    return { ok: true, settings: {} }
  }
})

ipcMain.handle('atlas:report', async (_event, { projectPath, options }) => {
  try {
    const format = options?.format || 'json'
    const out = await runCli(['report', '--project', projectPath, '--format', format], projectPath)
    return { ok: true, report: format === 'json' ? JSON.parse(out) : out }
  } catch (err) {
    return { ok: false, error: err.message }
  }
})

ipcMain.handle('atlas:close-project', async (_event, projectPath) => {
  const entry = mcpProcesses.get(projectPath)
  if (entry) {
    try {
      // Send shutdown notification
      const msg = JSON.stringify({ jsonrpc: '2.0', method: 'shutdown' }) + '\n'
      entry.proc.stdin.write(msg)
      entry.proc.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'exit' }) + '\n')
      entry.proc.stdin.end()
      entry.proc.kill('SIGTERM')
    } catch (_) {}
    mcpProcesses.delete(projectPath)
  }
  return { ok: true }
})