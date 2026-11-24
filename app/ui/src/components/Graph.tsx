import { useEffect, useRef, useState } from 'react'
import Graph from 'graphology'
import Sigma from 'sigma'
import forceAtlas2 from 'graphology-layout-forceatlas2'

interface GraphProps {
  nodes?: Array<{ id: string; label: string; kind: string; path: string; rank?: number }>
  edges?: Array<{ source: string; target: string }>
  selectedNode: string | null
}

export default function Graph({ nodes = [], edges = [], selectedNode }: GraphProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const sigmaRef = useRef<Sigma | null>(null)
  const graphRef = useRef<Graph | null>(null)
  const [layoutRunning, setLayoutRunning] = useState(false)

  useEffect(() => {
    if (!containerRef.current) return

    const graph = new Graph()
    graphRef.current = graph

    // Цвета по типу узла
    const colorFor = (kind: string) => {
      switch (kind) {
        case 'dir':
          return '#3b82f6'
        case 'file':
          return '#10b981'
        case 'class':
        case 'interface':
          return '#a855f7'
        case 'function':
        case 'method':
          return '#f59e0b'
        case 'constant':
          return '#ef4444'
        default:
          return '#6b7280'
      }
    }

    // Добавляем узлы
    nodes.forEach((node) => {
      graph.addNode(node.id, {
        label: node.label,
        path: node.path,
        kind: node.kind,
        color: colorFor(node.kind),
        size: node.kind === 'dir' ? 10 : node.kind === 'file' ? 8 : 6,
      })
    })

    // Добавляем рёбра
    edges.forEach((edge) => {
      if (graph.hasNode(edge.source) && graph.hasNode(edge.target)) {
        graph.addEdge(edge.source, edge.target, { size: 0.5 })
      }
    })

    // Инициализируем Sigma
    const sigma = new Sigma(graph, containerRef.current, {
      renderLabels: true,
      labelFont: 'Inter, system-ui',
      labelSize: 11,
      labelColor: '#374151',
      labelGrid: true,
      minCameraRatio: 0.1,
      maxCameraRatio: 10,
    })
    sigmaRef.current = sigma

    // Подсвечиваем выбранный узел
    if (selectedNode && graph.hasNode(selectedNode)) {
      graph.setNodeAttribute(selectedNode, 'highlighted', true)
      sigma.getCamera().animatedReset({ duration: 300 })
    }

    // Force-Atlas2 раскладка
    setLayoutRunning(true)
    const layout = forceAtlas2.infer(graph)
    const layoutInterval = setInterval(() => {
      layout.step()
      sigma.refresh()
    }, 16)

    return () => {
      clearInterval(layoutInterval)
      layout.kill()
      setLayoutRunning(false)
      sigma.kill()
    }
  }, [nodes, edges, selectedNode])

  // Обновляем выделение при смене selectedNode
  useEffect(() => {
    const graph = graphRef.current
    const sigma = sigmaRef.current
    if (!graph || !sigma) return

    graph.forEachNode((node) => {
      graph.setNodeAttribute(node, 'highlighted', false)
    })
    if (selectedNode && graph.hasNode(selectedNode)) {
      graph.setNodeAttribute(selectedNode, 'highlighted', true)
      sigma.getCamera().animatedReset({ duration: 300 })
    }
    sigma.refresh()
  }, [selectedNode])

  return (
    <div
      ref={containerRef}
      className="h-full w-full bg-background border-t relative overflow-hidden"
      style={{ backgroundColor: '#fafafa' }}
    >
      {nodes.length === 0 && !layoutRunning && (
        <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
          <div className="text-center">
            <div className="w-24 h-24 rounded-full border-2 border-blue-500 flex items-center justify-center mx-auto mb-4 animate-pulse">
              <div className="w-12 h-12 rounded-full bg-blue-500/20 flex items-center justify-center">
                <svg className="w-6 h-6 text-blue-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 6.253v13m0-13C10.832 8.088 9.246 9.424 7 11.268 5.12 13.526 3 16.488 3 20c0 4.469 2.61 8.49 6.545 10.966 2.123 1.364 5.8 1.364 7.923 0 3.935-2.477 6.545-6.497 6.545-10.966 0-3.512-2.12-6.474-4-8.736A10.01 10.01 0 0 0 20 13c0-2.29-.842 4.46-2.243-6.097C16.97 7.878 14.526 7 12 7z" />
                </svg>
              </div>
            </div>
            <h3 className="text-muted-foreground text-sm">Граф проекта</h3>
            <p className="text-[10px] text-muted-foreground mt-1">
              Узлы и связи появятся после индексации
            </p>
          </div>
        </div>
      )}

      {/* Инструкция управления */}
      <div className="absolute bottom-4 left-4 text-[10px] text-muted-foreground bg-background/80 backdrop-blur px-3 py-2 rounded border z-10">
        <div className="flex items-center gap-2 mb-1">
          <span className="w-2 h-2 rounded-full bg-blue-500"></span> Каталог
        </div>
        <div className="flex items-center gap-2 mb-1">
          <span className="w-2 h-2 rounded-full bg-green-500"></span> Файл
        </div>
        <div className="flex items-center gap-2 mb-1">
          <span className="w-2 h-2 rounded-full bg-purple-500"></span> Класс/Интерфейс
        </div>
        <div className="flex items-center gap-2 mb-1">
          <span className="w-2 h-2 rounded-full bg-orange-500"></span> Функция/Метод
        </div>
        <div className="mt-2 text-xs text-muted-foreground">
          Колесико — зум, перетаскивание — панорамирование
        </div>
      </div>
    </div>
  )
}
