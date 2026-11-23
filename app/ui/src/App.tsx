import { useState, useEffect } from 'react'
import { LayoutDashboard, FileText, GitBranch, Activity, BarChart2, Search, Settings, Zap, Copy, ExternalLink } from 'lucide-react'
import FileTree from './components/FileTree'
import Graph from './components/Graph'
import FileView from './components/FileView'
import Inspector from './components/Inspector'
import TokenPanel from './components/TokenPanel'

export default function App() {
  const [activeTab, setActiveTab] = useState<'graph' | 'file' | 'map' | 'report'>('graph')
  const [selectedNode, setSelectedNode] = useState<string | null>(null)
  const [sidebarOpen, setSidebarOpen] = useState(true)
  const [projects, setProjects] = useState<string[]>([])
  const [activeProject, setActiveProject] = useState<string | null>(null)

  const tabs = [
    { id: 'graph' as const, label: 'Граф', icon: <Activity className="w-4 h-4" /> },
    { id: 'file' as const, label: 'Файл', icon: <FileText className="w-4 h-4" /> },
    { id: 'map' as const, label: 'Карта', icon: <LayoutDashboard className="w-4 h-4" /> },
    { id: 'report' as const, label: 'Отчёт', icon: <BarChart2 className="w-4 h-4" /> },
  ]

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
              onChange={(e) => setActiveProject(e.target.value)}
            >
              <option value="">Выберите проект</option>
              {projects.map((p) => (
                <option key={p} value={p}>{p}</option>
              ))}
            </select>
          </div>

          <div className="flex items-center gap-2 ml-6">
            <button className="flex items-center gap-2 px-3 py-1.5 rounded border hover:bg-accent text-xs">
              <Search className="w-3 h-3" />
              Поиск
            </button>
            <button className="flex items-center gap-2 px-3 py-1.5 rounded border hover:bg-accent text-xs">
              <Copy className="w-3 h-3" />
              Подключить клиент
            </button>
            <button className="flex items-center gap-2 px-3 py-1.5 rounded border hover:bg-accent text-xs">
                <ExternalLink className="w-3 h-3" />
                Экспорт отчёта
              </button>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <div className="flex items-center gap-1 px-2 py-1 rounded bg-green-500/10 border border-green-500/20 text-[10px]">
            <span className="w-1.5 h-1.5 rounded-full bg-green-500"></span>
            Индекс: fresh
          </div>
          <div className="text-xs text-muted-foreground">0 токенов за сессию</div>
          <button className="px-3 py-1.5 rounded border hover:bg-accent text-xs">
            <Settings className="w-3 h-3" />
          </button>
        </div>
      </header>

      <div className="flex-1 flex overflow-hidden">
        {/* Левая боковая панель */}
        {sidebarOpen && (
          <aside className="w-80 border-r flex flex-col">
            <div className="p-3 border-b">
              <div className="flex items-center gap-2 mb-3">
                <FileText className="w-4 h-4" />
                <span className="font-medium text-sm">Файлы</span>
              </div>
              <div className="relative">
                <Search className="absolute left-2.5 top-2.5 w-3 h-3 text-muted-foreground" />
                <input
                  type="text"
                  placeholder="Поиск файлов и символов..."
                  className="w-full pl-8 pr-3 py-1.5 border rounded text-xs bg-input focus:outline-none focus:border-blue-500"
                />
              </div>
            </div>

            <FileTree
              projectPath={activeProject ?? ''}
              onNodeSelect={setSelectedNode}
            />
          </aside>
        )}

        {/* Центральная область */}
        <main className="flex-1 flex flex-col relative">
          <div className="flex-1 overflow-hidden bg-background">
            {activeTab === 'graph' && <Graph selectedNode={selectedNode} />}
            {activeTab === 'file' && <FileView nodeId={selectedNode} />}
            {activeTab === 'map' && <div className="h-full flex items-center justify-center">
              <div className="text-center">
                <LayoutDashboard className="w-12 h-12 text-muted-foreground mx-auto mb-2" />
                <p className="text-muted-foreground text-sm">Карта проекта — выберите модуль</p>
              </div>
            </div>}
            {activeTab === 'report' && <div className="h-full flex items-center justify-center">
              <div className="text-center">
                <BarChart2 className="w-12 h-12 text-muted-foreground mx-auto mb-2" />
                <p className="text-muted-foreground text-sm">Отчёт проекта — нажмите «Экспорт отчёта»</p>
              </div>
            </div>}
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

        {/* Правая боковая панель — инспектор */}
        <aside className="w-80 border-l flex flex-col">
          <Inspector nodeId={selectedNode} />
          <TokenPanel />
        </aside>
      </div>

      {/* Статусная строка */}
      <footer className="h-8 border-t bg-muted/50 flex items-center justify-between px-4 text-[10px] text-muted-foreground">
        <div className="flex items-center gap-4">
          <span>Индекс: fresh · 0 файлов, 0 узлов</span>
          <span>MCP-клиенты: 0</span>
          <span>Сессия: активна</span>
        </div>
        <div className="flex items-center gap-4">
          <span>Токенов за сессию: 0</span>
          <span>Экономия: 0%</span>
        </div>
      </footer>
    </div>
  )
}