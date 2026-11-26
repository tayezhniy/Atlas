import { useState, useEffect, useCallback } from 'react'
import {
  LayoutDashboard,
  FileText,
  Activity,
  BarChart2,
  Search,
  Settings,
  Zap,
  Copy,
  ExternalLink,
  Play,
  RefreshCw,
  Folder,
  AlertCircle,
  CheckCircle,
  X,
  File,
  ChevronLeft,
  ChevronRight,
} from 'lucide-react'
import FileTree from './components/FileTree'
import Graph from './components/Graph'
import FileView from './components/FileView'
import Inspector from './components/Inspector'
import TokenPanel from './components/TokenPanel'
import CommandPalette from './components/CommandPalette'
import SettingsPanel from './components/Settings'
import ReportExport from './components/ReportExport'
import type { GraphNode, ProjectOverview } from './types'

export default function App() {
  const [activeTab, setActiveTab] = useState<'graph' | 'file' | 'map' | 'report'>('graph')
  const [selectedNode, setSelectedNode] = useState<string | null>(null)
  const [sidebarOpen, setSidebarOpen] = useState(true)
  const [rightSidebarOpen, setRightSidebarOpen] = useState(true)
  const [projects, setProjects] = useState<string[]>([])
  const [activeProject, setActiveProject] = useState<string | null>(null)
  const [indexing, setIndexing] = useState(false)
  const [overview, setOverview] = useState<ProjectOverview | null>(null)
  const [graphNodes, setGraphNodes] = useState<GraphNode[]>([])
  const [graphEdges, setGraphEdges] = useState<ProjectOverview['edges']>([])
  const [error, setError] = useState<string | null>(null)
  const [status, setStatus] = useState<'idle' | 'indexing' | 'ready' | 'error'>('idle')
  const [recentFiles, setRecentFiles] = useState<string[]>([])

  // Модальные окна
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false)
  const [paletteMode, setPaletteMode] = useState<'files' | 'symbols' | 'commands'>('files')
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [reportExportOpen, setReportExportOpen] = useState(false)
  const [reportFormat, setReportFormat] = useState<'json' | 'markdown'>('markdown')

  const tabs = [
    { id: 'graph' as const, label: 'Граф', icon: <Activity className="w-4 h-4" /> },
    { id: 'file' as const, label: 'Файл', icon: <FileText className="w-4 h-4" /> },
    { id: 'map' as const, label: 'Карта', icon: <LayoutDashboard className="w-4 h-4" /> },
    { id: 'report' as const, label: 'Отчёт', icon: <BarChart2 className="w-4 h-4" /> },
  ]

  // Проверка жизнеспособности API при монтировании
  useEffect(() => {
    if (!window.atlasAPI) return
    window.atlasAPI.ping().then(() => setStatus('ready')).catch(() => setStatus('error'))

    // Глобальные хоткеи
    const handleKeyDown = (e: KeyboardEvent) => {
      // Ctrl+P - поиск файлов
      if ((e.ctrlKey || e.metaKey) && e.key === 'p' && !e.shiftKey) {
        e.preventDefault()
        setPaletteMode('files')
        setCommandPaletteOpen(true)
      }
      // Ctrl+Shift+F - поиск символов
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 'f') {
        e.preventDefault()
        setPaletteMode('symbols')
        setCommandPaletteOpen(true)
      }
      // Ctrl+K - команды
      if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault()
        setPaletteMode('commands')
        setCommandPaletteOpen(true)
      }
      // Ctrl+, - настройки
      if ((e.ctrlKey || e.metaKey) && e.key === ',') {
        e.preventDefault()
        setSettingsOpen(true)
      }
      // Ctrl+B - переключение левой боковой панели
      if ((e.ctrlKey || e.metaKey) && e.key === 'b') {
        e.preventDefault()
        setSidebarOpen(v => !v)
      }
      // Ctrl+Shift+B - переключение правой боковой панели
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 'b') {
        e.preventDefault()
        setRightSidebarOpen(v => !v)
      }
      // Escape - закрыть модалки
      if (e.key === 'Escape') {
        setCommandPaletteOpen(false)
        setSettingsOpen(false)
        setReportExportOpen(false)
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])

  const loadOverview = useCallback(async (projectPath: string) => {
    const ov = await window.atlasAPI.overview(projectPath)
    setOverview(ov)
    setGraphNodes(ov.top)
    setGraphEdges(ov.edges)
  }, [])

  const handleOpenProject = useCallback(async (projectPath: string) => {
    if (!window.atlasAPI) return
    setError(null)
    setStatus('indexing')
    setSelectedNode(null)
    try {
      await window.atlasAPI.openProject(projectPath)
      setProjects(prev => (prev.includes(projectPath) ? prev : [...prev, projectPath]))
      setActiveProject(projectPath)
      await loadOverview(projectPath)
      setStatus('ready')
      setRecentFiles([])
    } catch (err: any) {
      setError(err.message || 'Не удалось открыть проект')
      setStatus('error')
    }
  }, [loadOverview])

  const handleChooseProject = useCallback(async () => {
    const path = await window.atlasAPI?.chooseProject()
    if (path) await handleOpenProject(path)
  }, [handleOpenProject])

  const handleIndexProject = useCallback(async () => {
    if (!activeProject || !window.atlasAPI) return
    setIndexing(true)
    setError(null)
    try {
      await window.atlasAPI.indexProject(activeProject)
      await loadOverview(activeProject)
      setStatus('ready')
    } catch (err: any) {
      setError(err.message || 'Ошибка индексации')
      setStatus('error')
    } finally {
      setIndexing(false)
    }
  }, [activeProject, loadOverview])

  const handleNodeSelect = useCallback((nodeId: string) => {
    setSelectedNode(nodeId)
    setRecentFiles(prev => [nodeId.split('::')[0], ...prev.filter(p => p !== nodeId.split('::')[0])].slice(0, 10))
    setActiveTab('file')
  }, [])

  const handleGraphHighlight = useCallback((nodeId: string) => {
    setSelectedNode(nodeId)
    setActiveTab('graph')
  }, [])

  const handleGoToDefinition = useCallback((symbol: string) => {
    if (!activeProject || !window.atlasAPI) return
    window.atlasAPI.search({ projectPath: activeProject, query: symbol, limit: 10 })
      .then(res => {
        const match = res.items.find(i => i.name === symbol && i.kind !== 'file')
        if (match) handleNodeSelect(`${match.path}::${match.name}`)
      })
      .catch(err => setError(err.message))
  }, [activeProject, handleNodeSelect])

  const handleCommand = useCallback((command: string) => {
    switch (command) {
      case 'index':
        handleIndexProject()
        break
      case 'watch':
        if (activeProject) window.atlasAPI?.watch(activeProject).catch(err => setError(err.message))
        break
      case 'doctor':
        if (activeProject) {
          window.atlasAPI?.doctor(activeProject).then(r => alert(r.output)).catch(err => setError(err.message))
        }
        break
      case 'export-json':
      case 'export-md':
        setReportFormat(command === 'export-json' ? 'json' : 'markdown')
        setReportExportOpen(true)
        break
      case 'settings':
        setSettingsOpen(true)
        break
    }
  }, [activeProject, handleIndexProject])

  const copyClientConfig = async () => {
    if (!activeProject) return
    await navigator.clipboard.writeText(await window.atlasAPI.mcpConfig(activeProject))
    alert('Конфигурация MCP-сервера скопирована. Добавьте её в настройки клиента.')
  }

  return (
    <div className="atlas-app h-full overflow-hidden flex flex-col bg-background text-foreground">
      {/* Верхняя панель */}
      <header className="h-12 shrink-0 border-b bg-background/50 flex items-center justify-between gap-2 px-4 overflow-hidden whitespace-nowrap">
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <div className="w-6 h-6 rounded bg-blue-500 flex items-center justify-center">
              <Zap className="w-3 h-3 text-white" />
            </div>
            <span className="font-semibold tracking-tight">Atlas</span>
          </div>

          {/* Переключатель проектов */}
          <div className="flex items-center gap-2 ml-2">
                        <select
              className="border rounded px-2 py-1 text-xs bg-input w-40"
              value={activeProject ?? ''}
              onChange={(e) => {
                const p = e.target.value
                if (p) handleOpenProject(p)
              }}
            >
              <option value="">Выберите проект</option>
              {projects.map((p) => (
                <option key={p} value={p}>{p}</option>
              ))}
            </select>
            <button className="px-3 py-1 rounded border hover:bg-accent text-xs" onClick={handleChooseProject}>
              Открыть папку…
            </button>
          </div>

          {activeProject && (
            <div className="flex items-center gap-2 ml-2">
              <button
                className="flex items-center gap-2 px-3 py-1.5 rounded border hover:bg-accent text-xs disabled:opacity-50"
                onClick={handleIndexProject}
                disabled={indexing}
              >
                {indexing ? (
                  <>
                    <RefreshCw className="w-3 h-3 animate-spin" /> Индексация…
                  </>
                ) : (
                  <>
                    <Play className="w-3 h-3" /> Проиндексировать
                  </>
                )}
              </button>
              <button
                className="flex items-center gap-2 px-3 py-1.5 rounded border hover:bg-accent text-xs"
                onClick={() => { setPaletteMode('files'); setCommandPaletteOpen(true) }}
              >
                <Search className="w-3 h-3" />
                Поиск (Ctrl+P)
              </button>
              <button className="flex items-center gap-2 px-3 py-1.5 rounded border hover:bg-accent text-xs" onClick={copyClientConfig}>
                <Copy className="w-3 h-3" />
                Подключить клиент
              </button>
              <button
                className="flex items-center gap-2 px-3 py-1.5 rounded border hover:bg-accent text-xs"
                onClick={() => setReportExportOpen(true)}
              >
                <ExternalLink className="w-3 h-3" />
                Экспорт отчёта
              </button>
            </div>
          )}

          {!activeProject && (
            <div className="flex items-center gap-2 ml-2 text-xs text-muted-foreground">
              <AlertCircle className="w-3 h-3" />
              Проект не выбран
            </div>
          )}
        </div>

        <div className="flex items-center gap-3">
          {status === 'indexing' && (
            <div className="flex items-center gap-1 px-2 py-1 rounded bg-yellow-500/10 border border-yellow-500/20 text-[10px]">
              <RefreshCw className="w-1.5 h-1.5 animate-spin" />
              Индексация…
            </div>
          )}
          {status === 'ready' && (
            <div className="flex items-center gap-1 px-2 py-1 rounded bg-green-500/10 border border-green-500/20 text-[10px]">
              <span className="w-1.5 h-1.5 rounded-full bg-green-500"></span>
              Индекс готов
            </div>
          )}
          {status === 'error' && (
            <div className="flex items-center gap-1 px-2 py-1 rounded bg-red-500/10 border border-red-500/20 text-[10px]">
              <AlertCircle className="w-1.5 h-1.5" />
              Ошибка
            </div>
          )}
          {overview && (
            <div className="text-xs text-muted-foreground">
              {overview.files} файлов, {overview.symbols} символов
            </div>
          )}
          <button
            className="px-3 py-1.5 rounded border hover:bg-accent text-xs"
            onClick={() => setSettingsOpen(true)}
          >
            <Settings className="w-3 h-3" />
          </button>
        </div>
      </header>

      {error && (
        <div className="px-4 py-2 bg-red-50 border border-red-200 text-red-700 text-xs">
          {error}
        </div>
      )}

      <div className="flex-1 flex overflow-hidden">
        {/* Левая боковая панель */}
        {sidebarOpen && activeProject && (
          <aside className="w-80 border-r flex flex-col transition-all duration-200">
            <div className="p-3 border-b">
              <div className="flex items-center gap-2 mb-3">
                <Folder className="w-4 h-4" />
                <span className="font-medium text-sm">Файлы</span>
                <button
                  onClick={() => setSidebarOpen(false)}
                  className="ml-auto p-1 rounded hover:bg-muted text-muted-foreground"
                  title="Скрыть панель (Ctrl+B)"
                >
                  <ChevronLeft className="w-4 h-4" />
                </button>
              </div>
              <FileTree
                projectPath={activeProject}
                overview={overview}
                onNodeSelect={handleNodeSelect}
                onGraphHighlight={handleGraphHighlight}
                selectedNode={selectedNode}
              />
            </div>
          </aside>
        )}

        {/* Кнопка открытия левой панели когда она закрыта */}
        {!sidebarOpen && activeProject && (
          <div className="w-8 border-r flex items-center justify-center bg-muted/50">
            <button
              onClick={() => setSidebarOpen(true)}
              className="p-2 rounded hover:bg-muted text-muted-foreground"
              title="Показать панель файлов (Ctrl+B)"
            >
              <ChevronRight className="w-5 h-5" />
            </button>
          </div>
        )}

        {/* Центральная область */}
        <main className="flex-1 flex flex-col relative">
          <div className="flex-1 overflow-hidden bg-background">
            {activeTab === 'graph' && (
              <Graph
                nodes={graphNodes}
                edges={graphEdges}
                selectedNode={selectedNode}
                onNodeClick={handleNodeSelect}
              />
            )}
            {activeTab === 'file' && selectedNode && (
              <FileView
                key={`${activeProject}:${selectedNode}`}
                projectPath={activeProject ?? ''}
                nodeId={selectedNode}
                onGoToDefinition={handleGoToDefinition}
                onGraphHighlight={handleGraphHighlight}
              />
            )}
            {activeTab === 'file' && !selectedNode && (
              <div className="h-full flex items-center justify-center">
                <div className="text-center">
                  <FileText className="w-12 h-12 text-muted-foreground mx-auto mb-2" />
                  <p className="text-muted-foreground text-sm">Выберите символ или файл для просмотра</p>
                  <p className="text-xs text-muted-foreground mt-2">Или нажмите Ctrl+P для поиска</p>
                </div>
              </div>
            )}
            {activeTab === 'map' && overview && (
              <div className="h-full p-6 overflow-y-auto">
                <div className="text-center">
                  <LayoutDashboard className="w-12 h-12 text-muted-foreground mx-auto mb-2" />
                  <h3 className="text-sm font-semibold mb-2">Карта проекта</h3>
                  <p className="text-xs text-muted-foreground mb-4">
                    {overview.files} файлов · языки: {Object.entries(overview.langs).map(([l, n]) => `${l} ${n}`).join(', ')}
                  </p>
                  <div className="space-y-1 text-xs">
                    {overview.top.map((n) => (
                      <div key={n.path} className="flex items-center justify-between px-3 py-1 bg-muted/50 rounded cursor-pointer hover:bg-muted"
                        onClick={() => handleNodeSelect(n.path)}>
                        <span className="font-mono truncate">{n.path}</span>
                        <span className="text-muted-foreground">~{n.tokens_est ?? 0} ток.</span>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            )}
            {activeTab === 'report' && (
              <div className="h-full flex items-center justify-center">
                <div className="text-center">
                  <BarChart2 className="w-12 h-12 text-muted-foreground mx-auto mb-2" />
                  <p className="text-muted-foreground text-sm">Отчёт проекта — нажмите «Экспорт отчёта»</p>
                </div>
              </div>
            )}
          </div>

          {/* Табы в нижней части центральной области */}
          <div className="h-8 border-t flex">
            {tabs.map((tab) => (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={`flex-1 flex items-center justify-center gap-2 text-xs transition-colors ${
                  activeTab === tab.id
                    ? 'bg-accent text-accent-foreground border-b-2 border-primary'
                    : 'text-muted-foreground hover:bg-muted'
                }`}
              >
                {tab.icon}
                <span>{tab.label}</span>
              </button>
            ))}
          </div>
        </main>

        {/* Правая боковая панель — инспектор + токены */}
        {rightSidebarOpen && (
          <aside className="w-80 border-l flex flex-col transition-all duration-200">
            {selectedNode ? (
              <Inspector
                projectPath={activeProject ?? ''}
                nodeId={selectedNode}
                onGraphSelect={(nodeId) => {
                  setSelectedNode(nodeId)
                  setActiveTab('graph')
                }}
              />
            ) : (
              <div className="h-64 flex flex-col p-4">
                <div className="flex items-center gap-3 text-muted-foreground">
                  <File className="w-5 h-5 opacity-50" />
                  <span>Выберите узел для просмотра деталей</span>
                </div>
              </div>
            )}
            <TokenPanel projectPath={activeProject ?? ''} />
            <button
              onClick={() => setRightSidebarOpen(false)}
              className="p-2 mx-3 mb-2 rounded hover:bg-muted text-muted-foreground self-end"
              title="Скрыть панель (Ctrl+Shift+B)"
            >
              <ChevronRight className="w-5 h-5 rotate-180" />
            </button>
          </aside>
        )}

        {!rightSidebarOpen && (
          <div className="w-8 border-l flex items-center justify-center bg-muted/50">
            <button
              onClick={() => setRightSidebarOpen(true)}
              className="p-2 rounded hover:bg-muted text-muted-foreground"
              title="Показать инспектор (Ctrl+Shift+B)"
            >
              <ChevronLeft className="w-5 h-5 rotate-180" />
            </button>
          </div>
        )}
      </div>

      {/* Статусная строка */}
      <footer className="h-8 border-t bg-muted/50 flex items-center px-4 text-[10px] text-muted-foreground">
        <span>
          {overview ? `${overview.files} файлов · ${overview.symbols} символов` : 'Проект не открыт'}
        </span>
      </footer>

      {/* Модальные окна */}
      <CommandPalette
        projectPath={activeProject ?? ''}
        isOpen={commandPaletteOpen}
        initialMode={paletteMode}
        onClose={() => setCommandPaletteOpen(false)}
        onSelectFile={handleNodeSelect}
        onSelectSymbol={(symbol) => handleNodeSelect(`${symbol.path}::${symbol.name}`)}
        onRunCommand={handleCommand}
        recentFiles={recentFiles}
      />

      <SettingsPanel
        projectPath={activeProject ?? ''}
        isOpen={settingsOpen}
        onClose={() => setSettingsOpen(false)}
      />

      <ReportExport
        projectPath={activeProject ?? ''}
        isOpen={reportExportOpen}
        initialFormat={reportFormat}
        onClose={() => setReportExportOpen(false)}
      />
    </div>
  )
}