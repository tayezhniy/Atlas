import { useEffect, useMemo, useRef, useState } from 'react'
import Graph from 'graphology'
import Sigma from 'sigma'
import forceAtlas2 from 'graphology-layout-forceatlas2'
import { RotateCcw } from 'lucide-react'
import type { GraphEdge, GraphNode } from '../types'

interface GraphProps {
  nodes: GraphNode[]
  edges: GraphEdge[]
  selectedNode: string | null
  onNodeClick: (nodeId: string) => void
}

const EDGE_COLORS: Record<string, string> = {
  imports: '#3b82f6',
  calls: '#f59e0b',
  extends: '#a855f7',
  tests: '#10b981',
}

/** Граф файлов проекта. Узлы — пути файлов, рёбра — вызовы и импорты между ними. */
export default function GraphComponent({ nodes, edges, selectedNode, onNodeClick }: GraphProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const sigmaRef = useRef<Sigma | null>(null)
  const [hidden, setHidden] = useState<Set<string>>(new Set())
  const [hideTests, setHideTests] = useState(false)
  // Выбранный символ подсвечивает свой файл
  const selectedPath = selectedNode?.split('::')[0] ?? null
  const selectedRef = useRef<string | null>(selectedPath)
  selectedRef.current = selectedPath
  const clickRef = useRef(onNodeClick)
  clickRef.current = onNodeClick

  const graph = useMemo(() => {
    const g = new Graph()
    const pathById = new Map(nodes.map((n) => [n.id, n.path]))
    nodes
      .filter((n) => !hideTests || !/test|spec/i.test(n.path))
      .forEach((n, i) => {
        // Начальные координаты по спирали: раскладка воспроизводима
        const angle = i * 2.4
        const radius = Math.sqrt(i + 1)
        g.addNode(n.path, {
          label: n.name,
          x: radius * Math.cos(angle),
          y: radius * Math.sin(angle),
          size: 4 + 8 * (n.rank ?? 0),
          color: '#10b981',
        })
      })
    for (const e of edges) {
      const src = pathById.get(e.source)
      const dst = pathById.get(e.target)
      if (!src || !dst || hidden.has(e.kind) || !g.hasNode(src) || !g.hasNode(dst) || g.hasEdge(src, dst)) continue
      g.addEdge(src, dst, { color: EDGE_COLORS[e.kind] ?? '#9ca3af', size: e.confidence === 'guess' ? 0.5 : 1 })
    }
    if (g.order > 1) {
      forceAtlas2.assign(g, {
        iterations: g.order > 1000 ? 50 : 200,
        settings: forceAtlas2.inferSettings(g),
      })
    }
    return g
  }, [nodes, edges, hidden, hideTests])

  useEffect(() => {
    if (!containerRef.current || graph.order === 0) return
    const sigma = new Sigma(graph, containerRef.current, {
      labelSize: 11,
      nodeReducer: (node, data) =>
        node === selectedRef.current ? { ...data, color: '#2563eb', size: data.size * 1.6, forceLabel: true } : data,
    })
    sigma.on('clickNode', ({ node }) => clickRef.current(node))
    sigmaRef.current = sigma
    return () => {
      sigma.kill()
      sigmaRef.current = null
    }
  }, [graph])

  useEffect(() => {
    sigmaRef.current?.refresh()
  }, [selectedPath])

  const toggle = (kind: string) =>
    setHidden((prev) => {
      const next = new Set(prev)
      if (!next.delete(kind)) next.add(kind)
      return next
    })

  if (nodes.length === 0) {
    return (
      <div className="h-full w-full flex items-center justify-center text-sm text-muted-foreground">
        Откройте проект, чтобы увидеть граф
      </div>
    )
  }

  return (
    <div className="h-full w-full relative">
      <div ref={containerRef} className="absolute inset-0" style={{ backgroundColor: '#fafafa' }} />
      <div className="absolute top-2 left-2 z-10 flex items-center gap-3 bg-background border rounded-lg px-3 py-1.5 shadow-sm text-xs">
        {Object.entries(EDGE_COLORS).map(([kind, color]) => (
          <label key={kind} className="flex items-center gap-1 cursor-pointer">
            <input type="checkbox" checked={!hidden.has(kind)} onChange={() => toggle(kind)} />
            <span className="w-3 h-0.5" style={{ backgroundColor: color }} />
            {kind}
          </label>
        ))}
        <label className="flex items-center gap-1 cursor-pointer">
          <input type="checkbox" checked={hideTests} onChange={(e) => setHideTests(e.target.checked)} />
          Скрыть тесты
        </label>
        <button onClick={() => sigmaRef.current?.getCamera().animatedReset()} title="Сбросить вид">
          <RotateCcw className="w-3 h-3" />
        </button>
      </div>
      <div className="absolute bottom-2 right-2 z-10 bg-background border rounded px-2 py-1 text-[10px] text-muted-foreground">
        Файлов: {graph.order} · связей: {graph.size}
      </div>
    </div>
  )
}
