import { useCallback, useEffect, useState } from 'react'
import {
  Check, ChevronDown, FileText, Folder, GitBranch, Layers, PanelLeft, PanelRight, Plus,
  RefreshCw, Search, Settings, ShieldCheck, TerminalSquare, Waypoints, X, Plug,
} from 'lucide-react'
import FileTree from './components/FileTree'
import Graph from './components/Graph'
import FileView from './components/FileView'
import Inspector from './components/Inspector'
import ContextPanel from './components/ContextPanel'
import TokenPanel from './components/TokenPanel'
import CommandPalette from './components/CommandPalette'
import SettingsPanel from './components/Settings'
import ReportExport from './components/ReportExport'
import McpClients from './components/McpClients'
import { fileName, formatNumber, timeAgo } from './lib'
import type { IndexStatus, ProjectOverview } from './types'

type PaletteMode = 'files' | 'symbols' | 'commands'
const GRAPH = 'graph'
const ACTIVITY = 'activity'
const MCP = 'mcp'

export default function App() {
  const [projects, setProjects] = useState<string[]>([])
  const [project, setProject] = useState<string | null>(null)
  const [overview, setOverview] = useState<ProjectOverview | null>(null)
  const [status, setStatus] = useState<IndexStatus | null>(null)
  const [branch, setBranch] = useState<string | null>(null)
  const [version, setVersion] = useState(0)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [selected, setSelected] = useState<string | null>(null)
  const [openFiles, setOpenFiles] = useState<string[]>([])
  /** GRAPH, ACTIVITY или открытый `путь[::имя]` */
  const [tab, setTab] = useState<string>(GRAPH)
  const [graphMode, setGraphMode] = useState<'graph' | 'list'>('graph')
  const [leftOpen, setLeftOpen] = useState(true)
  const [rightOpen, setRightOpen] = useState(true)
  const [palette, setPalette] = useState<PaletteMode | null>(null)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [reportOpen, setReportOpen] = useState(false)
  const [reportFormat, setReportFormat] = useState<'json' | 'markdown'>('markdown')
  const [binaryPath, setBinaryPath] = useState<string>('')

  // Загрузить binary path при старте
  useEffect(() => {
    window.atlasAPI.binaryPath().then(setBinaryPath).catch(() => setBinaryPath('atlas-cli'))
  }, [])

  const projectName = project ? fileName(project) : ''
  const nodes = overview?.top ?? []
  const edges = overview?.edges ?? []

  const refresh = useCallback(async (path: string) => {
    const [ov, st, info] = await Promise.all([
      window.atlasAPI.overview(path),
      window.atlasAPI.status(path),
      window.atlasAPI.appInfo(path),
    ])
    setOverview(ov)
    setStatus(st)
    setBranch(info.branch)
    setVersion((v) => v + 1)
    return ov
  }, [])

  const openProject = useCallback(async (path: string) => {
    setBusy(true)
    setError(null)
    try {
      await window.atlasAPI.openProject(path)
      setProjects((prev) => (prev.includes(path) ? prev : [...prev, path]))
      setProject(path)
      setOverview(null)
      setSelected(null)
      setOpenFiles([])
      setTab(GRAPH)
      const ov = await refresh(path)
      // сразу показываем окрестность самого связанного файла
      const linked = new Set(ov.edges.flatMap((e) => [e.source, e.target]))
      setSelected((ov.top.find((n) => linked.has(n.id)) ?? ov.top[0])?.path ?? null)
    } catch (err: any) {
      setError(err.message || 'Не удалось открыть проект')
    } finally {
      setBusy(false)
    }
  }, [refresh])

  const chooseProject = useCallback(async () => {
    const path = await window.atlasAPI?.chooseProject()
    if (path) await openProject(path)
  }, [openProject])

  const reindex = useCallback(async () => {
    if (!project) return
    setBusy(true)
    setError(null)
    try {
      await window.atlasAPI.indexProject(project)
      await refresh(project)
    } catch (err: any) {
      setError(err.message || 'Ошибка индексации')
    } finally {
      setBusy(false)
    }
  }, [project, refresh])

  const openFile = useCallback((target: string) => {
    const path = target.split('::')[0]
    setSelected(target)
    setOpenFiles((prev) => (prev.includes(path) ? prev : [...prev, path]))
    setTab(target)
  }, [])

  const closeFile = (path: string) => {
    setOpenFiles((prev) => prev.filter((p) => p !== path))
    if (tab.split('::')[0] === path) setTab(GRAPH)
  }

  const goToDefinition = useCallback((symbol: string) => {
    if (!project) return
    window.atlasAPI
      .search({ projectPath: project, query: symbol, limit: 10 })
      .then((res) => {
        const match = res.items.find((i) => i.name === symbol && i.kind !== 'file')
        if (match) openFile(`${match.path}::${match.name}`)
      })
      .catch((err) => setError(err.message))
  }, [project, openFile])

  const runCommand = useCallback((command: string) => {
    if (command === 'index') reindex()
    else if (command === 'settings') setSettingsOpen(true)
    else if (command === 'export-json' || command === 'export-md') {
      setReportFormat(command === 'export-json' ? 'json' : 'markdown')
      setReportOpen(true)
    } else if (project && command === 'watch') {
      window.atlasAPI.watch(project).catch((err) => setError(err.message))
    } else if (project && command === 'doctor') {
      window.atlasAPI.doctor(project).then((r) => alert(r.output)).catch((err) => setError(err.message))
    }
  }, [project, reindex])

  const copyClientConfig = async () => {
    if (!project) return
    await navigator.clipboard.writeText(await window.atlasAPI.mcpConfig(project))
    alert('Конфигурация MCP-сервера скопирована. Добавьте её в настройки клиента.')
  }

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey
      const key = e.key.toLowerCase()
      const open = (mode: PaletteMode) => {
        e.preventDefault()
        setPalette(mode)
      }
      if (e.key === 'Escape') {
        setPalette(null)
        setSettingsOpen(false)
        setReportOpen(false)
      } else if (!mod) return
      else if (key === 'p') open('files')
      else if (e.shiftKey && key === 'f') open('symbols')
      else if (key === 'k') open('commands')
      else if (key === ',') setSettingsOpen(true)
      else if (key === 'b' && e.shiftKey) setRightOpen((v) => !v)
      else if (key === 'b') setLeftOpen((v) => !v)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  const lastCall = status?.calls.recent[0]
  const links = status ? Object.entries(status.edges).filter(([k]) => k !== 'contains').reduce((s, [, n]) => s + n, 0) : 0
  const mainLang = overview ? Object.entries(overview.langs).sort((a, b) => b[1] - a[1])[0]?.[0] : null
  const tabPath = tab.split('::')[0]

  const railButton = (title: string, icon: JSX.Element, onClick: () => void, active = false) => (
    <button key={title} title={title} onClick={onClick} className={`w-9 h-9 rounded-lg flex items-center justify-center ${active ? 'bg-accent text-accent-foreground' : 'text-muted-foreground hover:bg-muted'}`}>
      {icon}
    </button>
  )

  return (
    <div className="h-full flex flex-col overflow-hidden bg-background text-foreground">
      <header className="h-11 shrink-0 bg-panel border-b border-border flex items-center px-4 relative">
        <span className="text-lg font-semibold lowercase">atlas</span>
        {project && <span className="ml-3 text-xs text-muted-foreground">/&nbsp; {projectName}</span>}
        <span className="absolute left-1/2 -translate-x-1/2 text-xs text-muted-foreground whitespace-nowrap">Atlas — локальное рабочее пространство</span>
        <button className="ml-auto text-muted-foreground" onClick={() => setRightOpen((v) => !v)} title="Инспектор (Ctrl+Shift+B)">
          <PanelRight className="w-4 h-4" />
        </button>
      </header>

      {error && (
        <div className="px-4 py-2 bg-red-50 text-red-700 text-xs flex justify-between">
          <span>{error}</span>
          <button onClick={() => setError(null)}><X className="w-3 h-3" /></button>
        </div>
      )}

      <div className="flex-1 flex min-h-0">
        <nav className="w-[52px] shrink-0 bg-rail border-r border-border flex flex-col items-center py-3 gap-3">
          {railButton('Обозреватель (Ctrl+B)', <PanelLeft className="w-4 h-4" />, () => setLeftOpen((v) => !v))}
          {railButton('Граф проекта', <Waypoints className="w-4 h-4" />, () => { setTab(GRAPH); setGraphMode('graph') }, tab === GRAPH && graphMode === 'graph')}
          {railButton('Поиск символов (Ctrl+Shift+F)', <Search className="w-4 h-4" />, () => setPalette('symbols'))}
          {railButton('Список файлов', <Layers className="w-4 h-4" />, () => { setTab(GRAPH); setGraphMode('list') }, tab === GRAPH && graphMode === 'list')}
          {railButton('MCP и журнал вызовов', <TerminalSquare className="w-4 h-4" />, () => setTab(ACTIVITY), tab === ACTIVITY)}
          {railButton('Подключение MCP-клиентов', <Plug className="w-4 h-4" />, () => setTab(MCP), tab === MCP)}
          <div className="mt-auto">{railButton('Настройки (Ctrl+,)', <Settings className="w-4 h-4" />, () => setSettingsOpen(true))}</div>
        </nav>

        {leftOpen && (
          <aside className="w-[244px] shrink-0 bg-panel border-r border-border flex flex-col min-h-0 p-3">
            <div className="flex justify-between items-center px-1.5 mb-2 text-[11px] font-semibold text-muted-foreground">
              <span>ПРОЕКТ</span>
              <button onClick={chooseProject} title="Открыть папку…"><Plus className="w-4 h-4" /></button>
            </div>
            <label className="relative flex items-center gap-3 bg-muted rounded-lg px-3 py-2">
              <Folder className="w-4 h-4 shrink-0 text-accent-foreground" />
              <span className="min-w-0 flex-1">
                <span className="block text-sm truncate">{project ? projectName : 'Не выбран'}</span>
                <span className="block text-[10px] text-muted-foreground truncate" title={project ?? ''}>{project ?? 'Откройте папку проекта'}</span>
              </span>
              <ChevronDown className="w-3.5 h-3.5 shrink-0" />
              <select
                className="absolute inset-0 opacity-0 cursor-pointer"
                aria-label="Проект"
                value={project ?? ''}
                onChange={(e) => (e.target.value === '+' ? chooseProject() : e.target.value && openProject(e.target.value))}
              >
                <option value="">Выберите проект</option>
                {projects.map((p) => <option key={p} value={p}>{p}</option>)}
                <option value="+">Открыть папку…</option>
              </select>
            </label>

            <button onClick={() => setPalette('files')} className="mt-3 flex items-center gap-2 bg-background rounded-md px-3 py-2 text-xs text-muted-foreground">
              <Search className="w-3.5 h-3.5" /> Найти файл…
              <span className="ml-auto text-[10px]">Ctrl P</span>
            </button>

            <div className="flex justify-between px-1.5 mt-4 mb-2 text-[11px] font-semibold text-muted-foreground">
              <span>ОБОЗРЕВАТЕЛЬ</span>
              <span className="font-normal">{overview?.files}</span>
            </div>
            <div className="flex-1 overflow-auto min-h-0">
              {project && <FileTree key={project} projectName={projectName} nodes={nodes} selected={selected} onSelect={setSelected} onOpen={openFile} />}
            </div>

            {project && (
              <div className="mt-3 bg-muted rounded-lg p-3 text-[11px]">
                <div className="flex items-center gap-2 text-xs">
                  <Check className="w-3.5 h-3.5 text-green-600" />
                  {busy ? 'Индексация…' : status?.indexed_at ? 'Проиндексирован' : 'Нет индекса'}
                  <button onClick={reindex} disabled={busy} className="ml-auto" title="Обновить индекс">
                    <RefreshCw className={`w-3.5 h-3.5 ${busy ? 'animate-spin' : ''}`} />
                  </button>
                </div>
                <div className="text-muted-foreground mt-1">{overview?.files ?? 0} файлов · {formatNumber(links)} связей</div>
                <div className="text-muted-foreground opacity-70 mt-1">Обновлён: {timeAgo(status?.indexed_at)}</div>
              </div>
            )}
          </aside>
        )}

        <main className="flex-1 min-w-0 flex flex-col min-h-0">
          <div className="h-10 shrink-0 border-b border-border flex items-stretch overflow-x-auto text-xs">
            <button onClick={() => setTab(GRAPH)} className={`flex items-center gap-3 px-5 shrink-0 border-r border-border ${tab === GRAPH ? 'bg-muted' : 'text-muted-foreground'}`}>
              <Waypoints className="w-3.5 h-3.5 text-accent-foreground" /> Граф проекта
            </button>
            {openFiles.map((p) => (
              <div key={p} className={`flex items-center gap-2 px-4 shrink-0 border-r border-border ${tabPath === p ? 'bg-muted' : 'text-muted-foreground'}`}>
                <button className="flex items-center gap-2 h-full" onClick={() => openFile(p)} title={p}>
                  <FileText className="w-3.5 h-3.5" /> {fileName(p)}
                </button>
                <button onClick={() => closeFile(p)} title="Закрыть"><X className="w-3 h-3" /></button>
              </div>
            ))}
            <button className="px-4 text-muted-foreground" onClick={() => setPalette('files')} title="Открыть файл"><Plus className="w-4 h-4" /></button>
          </div>

          {!project ? (
            <div className="flex-1 flex flex-col items-center justify-center gap-3">
              <h1 className="text-xl font-semibold">Откройте проект</h1>
              <p className="text-xs text-muted-foreground">Atlas построит карту кода локально, на этом компьютере.</p>
              <button onClick={chooseProject} className="bg-primary text-primary-foreground rounded-md px-4 py-2 text-xs font-medium">Открыть папку…</button>
            </div>
          ) : tab === GRAPH ? (
            <>
              <div className="flex-1 min-h-0">
                <Graph
                  projectName={projectName}
                  nodes={nodes}
                  edges={edges}
                  totalFiles={overview?.files ?? 0}
                  selected={selected?.split('::')[0] ?? null}
                  mode={graphMode}
                  onModeChange={setGraphMode}
                  onSelect={setSelected}
                  onOpen={openFile}
                />
              </div>
              <ContextPanel projectPath={project} version={version} onExport={() => setReportOpen(true)} />
            </>
          ) : tab === ACTIVITY ? (
            <div className="flex-1 overflow-auto p-6 space-y-4">
              <h1 className="text-2xl font-semibold">MCP и журнал вызовов</h1>
              <p className="text-xs text-muted-foreground max-w-xl">
                Клиент нейросети запускает Atlas сам. Скопируйте конфигурацию и добавьте её в настройки клиента.
              </p>
              <div className="flex gap-3 text-xs">
                <button onClick={copyClientConfig} className="bg-primary text-primary-foreground rounded-md px-4 py-2 font-medium">Скопировать конфигурацию MCP</button>
                <button onClick={() => runCommand('doctor')} className="bg-muted rounded-md px-4 py-2">Проверить индекс</button>
              </div>
              <div className="max-w-xl border border-border rounded-lg"><TokenPanel key={version} projectPath={project} /></div>
            </div>
          ) : tab === MCP ? (
            <McpClients projectPath={project} binaryPath={binaryPath} />
          ) : (
            <div className="flex-1 min-h-0">
              <FileView
                key={`${project}:${tab}:${version}`}
                projectPath={project}
                nodeId={tab}
                onGoToDefinition={goToDefinition}
                onGraphHighlight={(id) => { setSelected(id); setTab(GRAPH) }}
              />
            </div>
          )}
        </main>

        {rightOpen && (
          <Inspector
            projectPath={project ?? ''}
            target={selected}
            nodes={nodes}
            edges={edges}
            version={version}
            onSelect={setSelected}
            onOpen={openFile}
            onClose={() => setRightOpen(false)}
          />
        )}
      </div>

      <footer className="h-8 shrink-0 bg-panel border-t border-border flex items-center gap-8 px-4 text-[11px] text-muted-foreground whitespace-nowrap">
        <span className="flex items-center gap-2"><ShieldCheck className="w-3.5 h-3.5 text-green-600" /> Индекс хранится локально, в .atlas/</span>
        {branch && <span className="flex items-center gap-2"><GitBranch className="w-3.5 h-3.5" /> {branch}</span>}
        <span className="ml-auto">{mainLang}</span>
        <span>{lastCall ? `Последний вызов MCP: ${lastCall.tool}, ${timeAgo(lastCall.time)}` : 'Вызовов MCP ещё не было'}</span>
      </footer>

      <CommandPalette
        projectPath={project ?? ''}
        isOpen={palette !== null}
        initialMode={palette ?? 'files'}
        onClose={() => setPalette(null)}
        onSelectFile={openFile}
        onSelectSymbol={(s) => openFile(`${s.path}::${s.name}`)}
        onRunCommand={runCommand}
        recentFiles={openFiles}
      />
      <SettingsPanel projectPath={project ?? ''} isOpen={settingsOpen} onClose={() => setSettingsOpen(false)} />
      <ReportExport projectPath={project ?? ''} isOpen={reportOpen} initialFormat={reportFormat} onClose={() => setReportOpen(false)} />
    </div>
  )
}
