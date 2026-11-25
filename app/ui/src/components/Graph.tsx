import { useEffect, useRef, useState, useCallback, useMemo } from 'react'
import Graph from 'graphology'
import Sigma from 'sigma'
import forceAtlas2 from 'graphology-layout-forceatlas2'
import { Activity, Filter, RotateCcw, Layers, Eye, EyeOff } from 'lucide-react'

interface GraphProps {
  nodes?: Array<{ id: string; name: string; kind: string; path: string; rank?: number; tokens_est?: number }>
  edges?: Array<{ source: string; target: string; kind?: string; confidence?: string }>
  selectedNode: string | null
  onNodeClick?: (nodeId: string | null) => void
  onNodeDoubleClick?: (nodeId: string) => void
}

type EdgeKind = 'contains' | 'imports' | 'calls' | 'extends' | 'tests' | 'mentions' | 'co_changes'
type Confidence = 'certain' | 'likely' | 'guess'

const EDGE_COLORS: Record<EdgeKind, string> = {
  contains: '#9ca3af',
  imports: '#3b82f6',
  calls: '#f59e0b',
  extends: '#a855f7',
  tests: '#10b981',
  mentions: '#ec4899',
  co_changes: '#6b7280',
}

const CONFIDENCE_STYLE: Record<Confidence, { dash: number[]; opacity: number }> = {
  certain: { dash: [], opacity: 1 },
  likely: { dash: [4, 4], opacity: 0.7 },
  guess: { dash: [2, 6], opacity: 0.4 },
}

const NODE_COLORS: Record<string, string> = {
  dir: '#3b82f6',
  file: '#10b981',
  module: '#3b82f6',
  class: '#a855f7',
  interface: '#a855f7',
  function: '#f59e0b',
  method: '#f59e0b',
  constant: '#ef4444',
  type: '#ec4899',
  test: '#10b981',
  doc: '#8b5cf6',
  config: '#6b7280',
}

const NODE_SIZES: Record<string, number> = {
  dir: 14,
  file: 12,
  module: 12,
  class: 10,
  interface: 10,
  function: 8,
  method: 8,
  constant: 6,
  type: 8,
  test: 7,
  doc: 8,
  config: 7,
}

export default function GraphComponent({
  nodes = [],
  edges = [],
  selectedNode,
  onNodeClick,
  onNodeDoubleClick,
}: GraphProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const sigmaRef = useRef<Sigma | null>(null)
  const graphRef = useRef<Graph | null>(null)
  const layoutRef = useRef<any>(null)
  const layoutIntervalRef = useRef<NodeJS.Timeout | null>(null)

  const [layoutRunning, setLayoutRunning] = useState(false)
  const [layoutPaused, setLayoutPaused] = useState(false)
  const [viewMode, setViewMode] = useState<'global' | 'local'>('global')
  const [filters, setFilters] = useState({
    kinds: new Set<string>(['dir', 'file', 'class', 'interface', 'function', 'method', 'constant', 'type', 'test']),
    edgeKinds: new Set<string>(['contains', 'imports', 'calls', 'extends', 'tests']),
    confidences: new Set<string>(['certain', 'likely', 'guess']),
    minRank: 0,
    hideTests: false,
    hideGenerated: false,
  })
  const [showEdgeLabels, setShowEdgeLabels] = useState(false)
  const [depth, setDepth] = useState(2)

  // Рефильтр узлов и рёбер
  const filteredNodes = useMemo(() => {
    return nodes.filter((node) => {
      if (!filters.kinds.has(node.kind)) return false
      if (filters.hideTests && node.kind === 'test') return false
      if (filters.hideGenerated && node.path?.includes('generated')) return false
      if ((node.rank ?? 0) < filters.minRank) return false
      return true
    })
  }, [nodes, filters])

  const filteredEdges = useMemo(() => {
    const nodeIds = new Set(filteredNodes.map(n => n.id))
    return edges.filter((edge) => {
      if (!nodeIds.has(edge.source) || !nodeIds.has(edge.target)) return false
      if (edge.kind && !filters.edgeKinds.has(edge.kind)) return false
      if (edge.confidence && !filters.confidences.has(edge.confidence)) return false
      return true
    })
  }, [edges, filteredNodes, filters])

  // Локальный граф: соседи выбранного узла до глубины depth
  const localGraphData = useMemo(() => {
    if (viewMode !== 'local' || !selectedNode) return { nodes: filteredNodes, edges: filteredEdges }

    const selected = nodes.find(n => n.id === selectedNode)
    if (!selected) return { nodes: filteredNodes, edges: filteredEdges }

    // BFS для поиска соседей
    const adj = new Map<string, Set<string>>()
    filteredEdges.forEach(e => {
      if (!adj.has(e.source)) adj.set(e.source, new Set())
      if (!adj.has(e.target)) adj.set(e.target, new Set())
      adj.get(e.source)!.add(e.target)
      adj.get(e.target)!.add(e.source)
    })

    const visited = new Set<string>()
    const queue = [{ id: selectedNode, dist: 0 }]
    visited.add(selectedNode)

    while (queue.length > 0) {
      const { id, dist } = queue.shift()!
      if (dist >= depth) continue
      const neighbors = adj.get(id)
      if (neighbors) {
        neighbors.forEach(n => {
          if (!visited.has(n)) {
            visited.add(n)
            queue.push({ id: n, dist: dist + 1 })
          }
        })
      }
    }

    const localNodes = filteredNodes.filter(n => visited.has(n.id))
    const localEdges = filteredEdges.filter(e => visited.has(e.source) && visited.has(e.target))

    return { nodes: localNodes, edges: localEdges }
  }, [viewMode, selectedNode, depth, filteredNodes, filteredEdges])

  // Инициализация графа
  const initGraph = useCallback(() => {
    if (!containerRef.current) return

    // Очистка предыдущего
    if (layoutIntervalRef.current) {
      clearInterval(layoutIntervalRef.current)
      layoutIntervalRef.current = null
    }
    if (layoutRef.current) {
      layoutRef.current.kill()
      layoutRef.current = null
    }
    if (sigmaRef.current) {
      sigmaRef.current.kill()
      sigmaRef.current = null
    }
    if (graphRef.current) {
      graphRef.current.clear()
    }

    const graph = new Graph()
    graphRef.current = graph

    // Добавляем узлы
    localGraphData.nodes.forEach((node) => {
      const kind = node.kind
      graph.addNode(node.id, {
        label: node.name,
        path: node.path,
        kind: node.kind,
        color: NODE_COLORS[kind] || '#6b7280',
        size: NODE_SIZES[kind] || 6,
        originalSize: NODE_SIZES[kind] || 6,
        rank: node.rank ?? 0,
        highlighted: node.id === selectedNode,
      })
    })

    // Добавляем рёбра
    localGraphData.edges.forEach((edge) => {
      if (graph.hasNode(edge.source) && graph.hasNode(edge.target)) {
        const eKind = (edge.kind as EdgeKind) || 'contains'
        const conf = (edge.confidence as Confidence) || 'certain'
        graph.addEdge(edge.source, edge.target, {
          kind: eKind,
          confidence: conf,
          color: EDGE_COLORS[eKind] || '#9ca3af',
          size: 1,
          originalColor: EDGE_COLORS[eKind] || '#9ca3af',
          dash: CONFIDENCE_STYLE[conf].dash,
          opacity: CONFIDENCE_STYLE[conf].opacity,
        })
      }
    })

    // Инициализируем Sigma
    const sigma = new Sigma(graph, containerRef.current, {
      renderLabels: true,
      labelFont: 'Inter, system-ui',
      labelSize: 11,
      labelColor: '#374151',
      labelGrid: true,
      minCameraRatio: 0.05,
      maxCameraRatio: 20,
      // Настройки рёбер
      edgeColor: 'source',
      defaultEdgeColor: '#9ca3af',
      edgeLabelSize: 10,
      edgeLabelColor: '#6b7280',
    })
    sigmaRef.current = sigma

    // Force-Atlas2 раскладка
    setLayoutRunning(true)
    layoutRef.current = forceAtlas2.assign(graph, {
      iterations: 500,
      settings: {
        gravity: 1,
        slowDown: 1,
        barnesHutOptimize: true,
        barnesHutTheta: 0.5,
        linLogMode: false,
        outboundAttractionDistribution: false,
        adjustSizes: false,
        edgeWeightInfluence: 1,
        scalingRatio: 10,
        strongGravityMode: false,
        barnesHut: true,
      },
    })

    layoutIntervalRef.current = setInterval(() => {
      if (!layoutPaused && layoutRef.current) {
        layoutRef.current.step()
        sigmaRef.current?.refresh()
      }
    }, 16)

    // Клики по узлам
    sigma.on('clickNode', (e) => {
      onNodeClick?.(e.node)
    })

    sigma.on('doubleClickNode', (e) => {
      onNodeDoubleClick?.(e.node)
    })

    sigma.on('clickStage', () => {
      onNodeClick?.(null)
    })

    // Подсветка выбранного узла
    if (selectedNode && graph.hasNode(selectedNode)) {
      graph.forEachNode((node) => {
        graph.setNodeAttribute(node, 'highlighted', node === selectedNode)
        graph.setNodeAttribute(node, 'size', node === selectedNode
          ? (graph.getNodeAttribute(node, 'originalSize') || 6) * 1.5
          : graph.getNodeAttribute(node, 'originalSize') || 6)
      })
      sigma.getCamera().animatedReset({ duration: 300 })
    }

    sigma.refresh()
  }, [localGraphData, selectedNode, layoutPaused, onNodeClick, onNodeDoubleClick])

  // Эффекты
  useEffect(() => {
    initGraph()
    return () => {
      if (layoutIntervalRef.current) clearInterval(layoutIntervalRef.current)
      if (layoutRef.current) layoutRef.current.kill()
      if (sigmaRef.current) sigmaRef.current.kill()
    }
  }, [initGraph])

  // Обновление подсветки при смене selectedNode
  useEffect(() => {
    const graph = graphRef.current
    const sigma = sigmaRef.current
    if (!graph || !sigma) return

    graph.forEachNode((node) => {
      const isSelected = node === selectedNode
      graph.setNodeAttribute(node, 'highlighted', isSelected)
      graph.setNodeAttribute(node, 'size', isSelected
        ? (graph.getNodeAttribute(node, 'originalSize') || 6) * 1.5
        : graph.getNodeAttribute(node, 'originalSize') || 6)
    })
    if (selectedNode && graph.hasNode(selectedNode)) {
      sigma.getCamera().animatedReset({ duration: 300 })
    }
    sigma.refresh()
  }, [selectedNode])

  // Обновление фильтров
  useEffect(() => {
    initGraph()
  }, [filters, viewMode, depth])

  // Toggle filter kind
  const toggleKind = (kind: string) => {
    setFilters(prev => {
      const next = new Set(prev.kinds)
      if (next.has(kind)) next.delete(kind)
      else next.add(kind)
      return { ...prev, kinds: next }
    })
  }

  const toggleEdgeKind = (kind: string) => {
    setFilters(prev => {
      const next = new Set(prev.edgeKinds)
      if (next.has(kind)) next.delete(kind)
      else next.add(kind)
      return { ...prev, edgeKinds: next }
    })
  }

  const toggleConfidence = (conf: string) => {
    setFilters(prev => {
      const next = new Set(prev.confidences)
      if (next.has(conf)) next.delete(conf)
      else next.add(conf)
      return { ...prev, confidences: next }
    })
  }

  const resetCamera = () => {
    sigmaRef.current?.getCamera().animatedReset({ duration: 500 })
  }

  const restartLayout = () => {
    setLayoutPaused(false)
    layoutRef.current = forceAtlas2.assign(graphRef.current!, {
      iterations: 500,
      settings: { gravity: 1, scalingRatio: 10, barnesHut: true },
    })
  }

  // Ключи для легенды
  const kinds = ['dir', 'file', 'class', 'interface', 'function', 'method', 'constant', 'type', 'test']
  const edgeKinds = ['contains', 'imports', 'calls', 'extends', 'tests']

  if (localGraphData.nodes.length === 0 && !layoutRunning) {
    return (
      <div className="h-full w-full flex items-center justify-center bg-background relative">
        <div className="text-center p-8">
          <div className="w-24 h-24 rounded-full border-2 border-blue-500 flex items-center justify-center mx-auto mb-4 animate-pulse">
            <div className="w-12 h-12 rounded-full bg-blue-500/20 flex items-center justify-center">
              <svg className="w-6 h-6 text-blue-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 6.253v13m0-13C10.832 8.088 9.246 9.424 7 11.268 5.12 13.526 3 16.488 3 20c0 4.469 2.61 8.49 6.545 10.966 2.123 1.364 5.8 1.364 7.923 0 3.935-2.477 6.545-6.497 6.545-10.966 0-3.512-2.12-6.474-4-8.736A10.01 10.01 0 0 0 20 13c0-2.29-.842 4.46-2.243-6.097C16.97 7.878 14.526 7 12 7z" />
              </svg>
            </div>
          </div>
          <h3 className="text-muted-foreground text-sm">Граф проекта</h3>
          <p className="text-[10px] text-muted-foreground mt-1">
            {nodes.length === 0 ? 'Проиндексируйте проект для отображения графа' : 'Нет узлов, соответствующих фильтрам'}
          </p>
        </div>
      </div>
    )
  }

  return (
    <div className="h-full w-full relative bg-background">
      {/* Канвас графа */}
      <div
        ref={containerRef}
        className="h-full w-full absolute inset-0"
        style={{ backgroundColor: '#fafafa' }}
      />

      {/* Панель управления сверху */}
      <div className="absolute top-2 left-2 right-2 z-10 flex flex-wrap gap-2 justify-center">
        {/* Режим вида */}
        <div className="flex items-center gap-1 bg-background/90 backdrop-blur rounded-lg border p-1 shadow-sm">
          <button
            onClick={() => setViewMode('global')}
            className={`px-3 py-1.5 text-xs rounded transition-colors ${
              viewMode === 'global' ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-muted'
            }`}
          >
            Глобальный
          </button>
          <button
            onClick={() => setViewMode('local')}
            disabled={!selectedNode}
            className={`px-3 py-1.5 text-xs rounded transition-colors ${
              viewMode === 'local' ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-muted'
            } disabled:opacity-50`}
          >
            Локальный (L)
          </button>
        </div>

        {/* Глубина локального графа */}
        {viewMode === 'local' && (
          <div className="flex items-center gap-2 bg-background/90 backdrop-blur rounded-lg border px-2 py-1 shadow-sm">
            <span className="text-xs text-muted-foreground">Глубина:</span>
            <select
              value={depth}
              onChange={e => setDepth(Number(e.target.value))}
              className="px-2 py-1 text-xs border rounded bg-background"
            >
              {[1, 2, 3, 4].map(d => <option key={d} value={d}>{d}</option>)}
            </select>
          </div>
        )}

        {/* Фильтры узлов */}
        <div className="relative">
          <button
            onClick={() => setFilters(p => ({ ...p, showKindFilter: !p.showKindFilter }))}
            className="flex items-center gap-1 px-3 py-1.5 text-xs bg-background/90 backdrop-blur rounded-lg border shadow-sm"
          >
            <Filter className="w-3 h-3" /> Узлы
          </button>
          {filters.showKindFilter && (
            <div className="absolute top-full left-0 mt-1 bg-background border rounded-lg shadow-lg p-2 z-20 min-w-[180px]">
              {kinds.map(kind => (
                <label key={kind} className="flex items-center gap-2 px-2 py-1 text-xs hover:bg-muted rounded cursor-pointer">
                  <input
                    type="checkbox"
                    checked={filters.kinds.has(kind)}
                    onChange={() => toggleKind(kind)}
                    className="w-3 h-3 accent-blue-500"
                  />
                  <span className="w-2 h-2 rounded-full" style={{ backgroundColor: NODE_COLORS[kind] }} />
                  <span className="capitalize">{kind}</span>
                </label>
              ))}
            </div>
          )}
        </div>

        {/* Фильтры рёбер */}
        <div className="relative">
          <button
            onClick={() => setFilters(p => ({ ...p, showEdgeFilter: !p.showEdgeFilter }))}
            className="flex items-center gap-1 px-3 py-1.5 text-xs bg-background/90 backdrop-blur rounded-lg border shadow-sm"
          >
            <Layers className="w-3 h-3" /> Рёбра
          </button>
          {filters.showEdgeFilter && (
            <div className="absolute top-full left-0 mt-1 bg-background border rounded-lg shadow-lg p-2 z-20 min-w-[180px]">
              {edgeKinds.map(kind => (
                <label key={kind} className="flex items-center gap-2 px-2 py-1 text-xs hover:bg-muted rounded cursor-pointer">
                  <input
                    type="checkbox"
                    checked={filters.edgeKinds.has(kind)}
                    onChange={() => toggleEdgeKind(kind)}
                    className="w-3 h-3 accent-blue-500"
                  />
                  <span className="w-4 h-0.5" style={{ backgroundColor: EDGE_COLORS[kind as EdgeKind] }} />
                  <span className="capitalize">{kind}</span>
                </label>
              ))}
              <div className="border-t my-1" />
              {['certain', 'likely', 'guess'].map(conf => (
                <label key={conf} className="flex items-center gap-2 px-2 py-1 text-xs hover:bg-muted rounded cursor-pointer">
                  <input
                    type="checkbox"
                    checked={filters.confidences.has(conf)}
                    onChange={() => toggleConfidence(conf)}
                    className="w-3 h-3 accent-blue-500"
                  />
                  <span className="capitalize text-[10px]">{conf}</span>
                  <span className="w-6 h-0.5 ml-auto" style={{
                    backgroundColor: '#6b7280',
                    borderTop: CONFIDENCE_STYLE[conf as Confidence].dash.length ? '1px dashed #6b7280' : '1px solid #6b7280'
                  }} />
                </label>
              ))}
            </div>
          )}
        </div>

        {/* Экстра фильтры */}
        <div className="flex items-center gap-2 bg-background/90 backdrop-blur rounded-lg border px-2 py-1 shadow-sm">
          <label className="flex items-center gap-1 text-xs cursor-pointer">
            <input
              type="checkbox"
              checked={filters.hideTests}
              onChange={e => setFilters(p => ({ ...p, hideTests: e.target.checked }))}
              className="w-3 h-3 accent-blue-500"
            />
            <Eye className="w-3 h-3" /> Скрыть тесты
          </label>
          <label className="flex items-center gap-1 text-xs cursor-pointer">
            <input
              type="checkbox"
              checked={filters.hideGenerated}
              onChange={e => setFilters(p => ({ ...p, hideGenerated: e.target.checked }))}
              className="w-3 h-3 accent-blue-500"
            />
            <Eye className="w-3 h-3" /> Скрыть generated
          </label>
        </div>

        {/* Управление камерой */}
        <div className="flex items-center gap-1 bg-background/90 backdrop-blur rounded-lg border p-1 shadow-sm">
          <button onClick={resetCamera} className="p-1.5 hover:bg-muted rounded" title="Сбросить вид">
            <RotateCcw className="w-3 h-3" />
          </button>
          <button onClick={restartLayout} className="p-1.5 hover:bg-muted rounded" title="Пересчитать раскладку" disabled={layoutPaused}>
            <Activity className="w-3 h-3" />
          </button>
          <button onClick={() => setLayoutPaused(!layoutPaused)} className="p-1.5 hover:bg-muted rounded" title={layoutPaused ? 'Возобновить' : 'Пауза'}>
            {layoutPaused ? <EyeOff className="w-3 h-3" /> : <Eye className="w-3 h-3" />}
          </button>
        </div>
      </div>

      {/* Легенда снизу */}
      <div className="absolute bottom-2 left-2 right-2 z-10 flex flex-wrap gap-2 justify-center">
        <div className="bg-background/90 backdrop-blur rounded-lg border px-3 py-2 shadow-sm flex flex-wrap items-center gap-3 text-[10px]">
          <div className="flex items-center gap-1">
            <span className="w-2 h-2 rounded-full" style={{ backgroundColor: NODE_COLORS.dir }} />
            <span>Каталог</span>
          </div>
          <div className="flex items-center gap-1">
            <span className="w-2 h-2 rounded-full" style={{ backgroundColor: NODE_COLORS.file }} />
            <span>Файл</span>
          </div>
          <div className="flex items-center gap-1">
            <span className="w-2 h-2 rounded-full" style={{ backgroundColor: NODE_COLORS.class }} />
            <span>Класс</span>
          </div>
          <div className="flex items-center gap-1">
            <span className="w-2 h-2 rounded-full" style={{ backgroundColor: NODE_COLORS.interface }} />
            <span>Интерфейс</span>
          </div>
          <div className="flex items-center gap-1">
            <span className="w-2 h-2 rounded-full" style={{ backgroundColor: NODE_COLORS.function }} />
            <span>Функция</span>
          </div>
          <div className="flex items-center gap-1">
            <span className="w-2 h-2 rounded-full" style={{ backgroundColor: NODE_COLORS.method }} />
            <span>Метод</span>
          </div>
          <div className="flex items-center gap-1">
            <span className="w-2 h-2 rounded-full" style={{ backgroundColor: NODE_COLORS.constant }} />
            <span>Константа</span>
          </div>
          <div className="flex items-center gap-1">
            <span className="flex items-center gap-1">
              <span className="w-4 h-0.5" style={{ backgroundColor: '#6b7280' }} />
              <span className="ml-1">certain</span>
            </span>
          </div>
          <div className="flex items-center gap-1">
            <span className="flex items-center gap-1">
              <span className="w-4 h-0.5 border-t" style={{ borderTop: '2px dashed #6b7280' }} />
              <span className="ml-1">likely</span>
            </span>
          </div>
          <div className="flex items-center gap-1">
            <span className="flex items-center gap-1">
              <span className="w-4 h-0.5 border-t" style={{ borderTop: '2px dotted #6b7280' }} />
              <span className="ml-1">guess</span>
            </span>
          </div>
        </div>
      </div>

      {/* Статистика */}
      <div className="absolute top-2 right-2 z-10 bg-background/90 backdrop-blur rounded-lg border px-3 py-1.5 shadow-sm text-[10px] text-muted-foreground">
        Узлов: {localGraphData.nodes.length} | Рёбер: {localGraphData.edges.length}
        {layoutRunning && <span className="ml-2"> | Раскладка...</span>}
      </div>
    </div>
  )
}