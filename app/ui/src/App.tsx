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
import type { GraphNode, ProjectOverview, SearchResult } from './types'

export default function App() {
  const [activeTab, setActiveTab] = useState<'graph' | 'file' | 'map' | 'report'>('graph')
  const [selectedNode, setSelectedNode] = useState<string | null>(null)
  const [sidebarOpen, setSidebarOpen] = useState(true)
  const [rightSidebarOpen, setRightSidebarOpen] = useState(true)
  const [projects, setProjects] = useState<string[]>([])
  const [activeProject, setActiveProject] = useState<string | null>(null)
  const [indexing, setIndexing] = useState(false)
  const [indexed, setIndexed] = useState(false)
  const [overview, setOverview] = useState<ProjectOverview | null>(null)
  const [graphNodes, setGraphNodes] = useState<GraphNode[]>([])
  const [graphEdges, setGraphEdges] = useState<Array<{ source: string; target: string }>>([])
  const [error, setError] = useState<string | null>(null)
  const [status, setStatus] = useState<'idle' | 'indexing' | 'ready' | 'error'>('idle')
  const [recentFiles, setRecentFiles] = useState<string[]>([])

  // Модальные окна
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [reportExportOpen, setReportExportOpen] = useState(false)

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
        setCommandPaletteOpen(true)
      }
      // Ctrl+Shift+F - поиск символов
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'f') {
        e.preventDefault()
        setCommandPaletteOpen(true)
      }
      // Ctrl+K - команды
      if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault()
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
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'b') {
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

  const handleOpenProject = useCallback(async (projectPath: string) => {
    if (!window.atlasAPI) return
    setError(null)
    setStatus('indexing')
    try {
      await window.atlasAPI.openProject(projectPath)
      setProjects(prev => (prev.includes(projectPath) ? prev : [...prev, projectPath]))
      setActiveProject(projectPath)
      setIndexed(true)
      await loadOverview(projectPath)
      setStatus('ready')
      // Добавляем в недавние файлы
      setRecentFiles(prev => [projectPath, ...prev.filter(p => p !== projectPath)].slice(0, 10))
    } catch (err: any) {
      setError(err.message || 'Не удалось открыть проект')
      setStatus('error')
    }
  }, [])

  const handleIndexProject = useCallback(async () => {
    if (!activeProject || !window.atlasAPI) return
    setIndexing(true)
    setError(null)
    try {
      const out = await window.atlasAPI.indexProject(activeProject)
      setIndexed(true)
      await loadOverview(activeProject)
    } catch (err: any) {
      setError(err.message || 'Ошибка индексации')
      setStatus('error')
    } finally {
      setIndexing(false)
    }
  }, [activeProject])

  const loadOverview = useCallback(async (projectPath: string) => {
    if (!window.atlasAPI) return
    try {
      const ov = await window.atlasAPI.overview(projectPath)
      setOverview(ov)
      const nodes: GraphNode[] = ov.top.map((n) => ({
        id: n.path,
        name: n.name,
        kind: 'file',
        path: n.path,
        rank: n.rank,
        tokens_est: n.tokens_est,
      }))
      setGraphNodes(nodes)
      setGraphEdges([])
    } catch (_) {
      // обзор недоступен
    }
  }, [])

  const handleSearch = useCallback(async (query: string) => {
    if (!activeProject || !window.atlasAPI || !query.trim()) return
    try {
      const res = await window.atlasAPI.search({
        projectPath: activeProject,
        query,
        limit: 20,
      })
      setSearchResults(res.items)
    } catch (err: any) {
      setError(err.message || 'Ошибка поиска')
    }
  }, [activeProject])

  const [searchResults, setSearchResults] = useState<SearchResult[]>([])

  const handleNodeSelect = useCallback((nodeId: string | null, nodeType?: 'file' | 'symbol') => {
    setSelectedNode(nodeId)
    if (nodeId && activeTab !== 'file') setActiveTab('file')
  }, [activeTab])

  const handleGraphHighlight = useCallback((nodeId: string) => {
    // Граф сам обработает подсветку через selectedNode
    setSelectedNode(nodeId)
    if (activeTab !== 'graph') setActiveTab('graph')
  }, [activeTab])

  const handleGoToDefinition = useCallback((symbol: string, path?: string) => {
    if (!activeProject || !window.atlasAPI) return
    // Ищем символ через search
    window.atlasAPI.search({ projectPath: activeProject, query: symbol, limit: 5 })
      .then(res => {
        if (res.items.length > 0) {
          const match = path
            ? res.items.find(i => i.path === path)
            : res.items[0]
          if (match) {
            const symbolId = `${match.path}::${match.name}`
            setSelectedNode(symbolId)
            setActiveTab('file')
          }
        }
      })
  }, [activeProject])

  const handleCommand = useCallback((command: string) => {
    switch (command) {
      case 'index':
        handleIndexProject()
        break
      case 'watch':
        if (activeProject && window.atlasAPI) {
          // Запускаем watch через CLI (нужно добавить в API)
        }
        break
      case 'doctor':
        if (activeProject && window.atlasAPI) {
          window.atlasAPI.doctor(activeProject).then(r => {
            alert(r.output || JSON.stringify(r.result))
          })
        }
        break
      case 'export-json':
        setReportExportOpen(true)
        // Формат JSON будет выбран по умолчанию
        break
      case 'export-md':
        setReportExportOpen(true)
        // Переключим на markdown после открытия
        break
      case 'settings':
        setSettingsOpen(true)
        break
    }
  }, [activeProject, handleIndexProject])

  return (
    <div className="atlas-app h-screen flex flex-col bg-background text-foreground">
      {/* Верхняя панель */}
      <header className="h-12 border-b bg-background/50 flex items-center justify-between px-4">
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <div className="w-6 h-6 rounded bg-blue-500 flex items-center justify-center">
              <Zap className="w-3 h-3 text-white" />
            </div>
            <span className="font-semibold tracking-tight">Atlas</span>
          </div>

          {/* Переключатель проектов */}
          <div className="flex items-center gap-2 ml-6">
            <label className="text-xs text-muted-foreground">Проект:</label>
            <select
              className="border rounded px-2 py-1 text-xs bg-input"
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
          </div>

          {activeProject && (
            <div className="flex items-center gap-2 ml-6">
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
                onClick={() => setCommandPaletteOpen(true)}
              >
                <Search className="w-3 h-3" />
                Поиск (Ctrl+P)
              </button>
              <button className="flex items-center gap-2 px-3 py-1.5 rounded border hover:bg-accent text-xs">
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
            <div className="flex items-center gap-2 ml-6 text-xs text-muted-foreground">
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
              Индекс: fresh
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
              {overview.files} файлов, {overview.top.length} символов
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
                onNodeDoubleClick={(nodeId) => { handleNodeSelect(nodeId); setActiveTab('file'); }}
              />
            )}
            {activeTab === 'file' && selectedNode && (
              <FileView
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
                        onClick={() => { handleNodeSelect(n.path); setActiveTab('file'); }}>
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
            <TokenPanel projectPath={activeProject ?? ''} overview={overview} />
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
      <footer className="h-8 border-t bg-muted/50 flex items-center justify-between px-4 text-[10px] text-muted-foreground">
        <div className="flex items-center gap-4">
          <span>
            Индекс: {status === 'ready' ? 'fresh' : 'пуст'} ·
            {overview?.files ?? 0} файлов ·
            {overview?.top.length ?? 0} символов
          </span>
          <span>MCP-клиенты: 0</span>
          <span>Сессия: {status}</span>
        </div>
        <div className="flex items-center gap-4">
          <span>Токенов за сессию: 0</span>
          <span>Экономия: 0%</span>
        </div>
      </footer>

      {/* Модальные окна */}
      <CommandPalette
        projectPath={activeProject ?? ''}
        isOpen={commandPaletteOpen}
        onClose={() => setCommandPaletteOpen(false)}
        onSelectFile={(path) => { handleNodeSelect(path, 'file'); setActiveTab('file'); }}
        onSelectSymbol={(symbol) => {
          const symbolId = `${symbol.path}::${symbol.name}`
          handleNodeSelect(symbolId, 'symbol')
          setActiveTab('file')
        }}
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
        overview={overview}
        graphNodes={graphNodes}
        isOpen={reportExportOpen}
        onClose={() => setReportExportOpen(false)}
      />
    </div>
  )
}