import { useEffect, useMemo, useRef, useState } from 'react'
import Graph from 'graphology'
import forceAtlas2 from 'graphology-layout-forceatlas2'
import { ChevronDown, Maximize, Minus, Plus, SlidersHorizontal } from 'lucide-react'
import type { GraphEdge, GraphNode } from '../types'
import { dirName, fileName, groupsOf, OTHER_COLOR } from '../lib'

interface GraphProps {
  projectName: string
  nodes: GraphNode[]
  edges: GraphEdge[]
  totalFiles: number
  selected: string | null
  mode: 'graph' | 'list'
  onModeChange: (mode: 'graph' | 'list') => void
  onSelect: (path: string) => void
  onOpen: (path: string) => void
}

const KINDS: Record<string, string> = { imports: 'импорты', calls: 'вызовы', extends: 'наследование', tests: 'тесты' }
const MAX_VISIBLE = 160
const WIDTH = 920
const HEIGHT = 480

export default function GraphComponent(props: GraphProps) {
  const { projectName, nodes, edges, totalFiles, selected, mode, onModeChange, onSelect, onOpen } = props
  const [hidden, setHidden] = useState<Set<string>>(new Set())
  const [hideTests, setHideTests] = useState(false)
  const [showGuess, setShowGuess] = useState(false)
  const [filtersOpen, setFiltersOpen] = useState(false)
  const [depth, setDepth] = useState(2)
  const [view, setView] = useState({ x: 0, y: 0, k: 1 })
  const drag = useRef<{ x: number; y: number } | null>(null)
  const svgRef = useRef<SVGSVGElement>(null)
  const groups = useMemo(() => groupsOf(nodes), [nodes])

  const links = useMemo(() => {
    const byId = new Map(nodes.map((n) => [n.id, n.path]))
    const pairs = new Map<string, [string, string]>()
    for (const e of edges) {
      const a = byId.get(e.source)
      const b = byId.get(e.target)
      if (a && b && !hidden.has(e.kind) && (showGuess || e.confidence !== 'guess')) pairs.set([a, b].sort().join('\n'), [a, b])
    }
    return [...pairs.values()]
  }, [nodes, edges, hidden, showGuess])

  const layout = useMemo(() => {
    const allowed = nodes.filter((n) => !hideTests || !/test|spec/i.test(n.path))
    const adjacent = new Map<string, string[]>()
    for (const [a, b] of links) {
      adjacent.set(a, [...(adjacent.get(a) ?? []), b])
      adjacent.set(b, [...(adjacent.get(b) ?? []), a])
    }
    // Локальный граф: окрестность выбранного файла заданной глубины
    let visible = allowed
    if (selected && depth > 0 && allowed.some((n) => n.path === selected)) {
      const near = new Set([selected])
      let frontier = [selected]
      for (let i = 0; i < depth; i++) {
        frontier = frontier.flatMap((p) => adjacent.get(p) ?? []).filter((p) => !near.has(p))
        frontier.forEach((p) => near.add(p))
      }
      visible = allowed.filter((n) => near.has(n.path))
    }
    // nodes уже отсортированы по рангу: при переполнении остаются главные
    const shown = visible.slice(0, MAX_VISIBLE)
    const sel = visible.find((n) => n.path === selected)
    if (sel && !shown.includes(sel)) shown[shown.length - 1] = sel

    const g = new Graph()
    shown.forEach((n, i) => {
      const angle = i * 2.4
      g.addNode(n.path, { x: Math.sqrt(i + 1) * Math.cos(angle), y: Math.sqrt(i + 1) * Math.sin(angle) })
    })
    const kept = links.filter(([a, b]) => a !== b && g.hasNode(a) && g.hasNode(b))
    kept.forEach(([a, b]) => g.hasEdge(a, b) || g.addEdge(a, b))
    if (g.order > 1) {
      forceAtlas2.assign(g, { iterations: 250, settings: { ...forceAtlas2.inferSettings(g), gravity: 1.5 } })
    }
    const points = shown.map((n) => g.getNodeAttributes(n.path) as { x: number; y: number })
    const xs = points.map((p) => p.x)
    const ys = points.map((p) => p.y)
    const [minX, maxX, minY, maxY] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)]
    const fit = (v: number, min: number, max: number, size: number) =>
      max - min < 1e-6 ? size / 2 : 60 + ((v - min) / (max - min)) * (size - 120)
    const position = new Map(
      shown.map((n, i) => [n.path, { x: fit(points[i].x, minX, maxX, WIDTH), y: fit(points[i].y, minY, maxY, HEIGHT) }]),
    )
    const degree = new Map<string, number>()
    kept.flat().forEach((p) => degree.set(p, (degree.get(p) ?? 0) + 1))
    return { shown, kept, position, degree, available: visible.length }
  }, [nodes, links, selected, depth, hideTests])

  useEffect(() => setView({ x: 0, y: 0, k: 1 }), [selected, depth, nodes])

  const zoom = (factor: number) =>
    setView((v) => {
      const k = Math.min(4, Math.max(0.4, v.k * factor))
      const r = k / v.k
      return { k, x: WIDTH / 2 - (WIDTH / 2 - v.x) * r, y: HEIGHT / 2 - (HEIGHT / 2 - v.y) * r }
    })

  const near = useMemo(
    () => new Set(layout.kept.filter((l) => selected && l.includes(selected)).flat()),
    [layout, selected],
  )
  const labelled = (path: string, i: number) => layout.shown.length <= 40 || i < 14 || near.has(path)
  const parts = [projectName, ...(selected ? dirName(selected).split('/').filter(Boolean) : [])]
  const toggle = (kind: string) =>
    setHidden((prev) => {
      const next = new Set(prev)
      if (!next.delete(kind)) next.add(kind)
      return next
    })
  const activeFilters = hidden.size + (hideTests ? 1 : 0)
  const control = 'bg-muted rounded-md border border-border'

  return (
    <div className="h-full flex flex-col min-h-0">
      <div className="flex items-start justify-between px-6 pt-5">
        <div>
          <h1 className="text-2xl font-semibold">Карта проекта</h1>
          <div className="text-xs text-muted-foreground mt-1">{parts.join('  /  ')}</div>
        </div>
        <div className="flex gap-3 relative">
          <div className={`${control} flex p-0.5`}>
            {(['graph', 'list'] as const).map((m) => (
              <button
                key={m}
                onClick={() => onModeChange(m)}
                className={`px-4 py-1.5 text-xs rounded ${mode === m ? 'bg-accent text-accent-foreground' : 'text-muted-foreground'}`}
              >
                {m === 'graph' ? 'Граф' : 'Список'}
              </button>
            ))}
          </div>
          <button className={`${control} flex items-center gap-2 px-3 text-xs`} onClick={() => setFiltersOpen((v) => !v)}>
            <SlidersHorizontal className="w-3.5 h-3.5" /> Фильтры
            {activeFilters > 0 && <span className="bg-accent text-accent-foreground rounded px-1.5">{activeFilters}</span>}
          </button>
          {filtersOpen && (
            <div className="absolute right-0 top-10 z-20 w-48 bg-panel border border-border rounded-md p-3 space-y-2 text-xs shadow-xl">
              {Object.entries(KINDS).map(([kind, label]) => (
                <label key={kind} className="flex items-center gap-2 cursor-pointer">
                  <input type="checkbox" checked={!hidden.has(kind)} onChange={() => toggle(kind)} /> {label}
                </label>
              ))}
              <label className="flex items-center gap-2 cursor-pointer border-t border-border pt-2">
                <input type="checkbox" checked={showGuess} onChange={(e) => setShowGuess(e.target.checked)} /> предположительные связи
              </label>
              <label className="flex items-center gap-2 cursor-pointer">
                <input type="checkbox" checked={hideTests} onChange={(e) => setHideTests(e.target.checked)} /> скрыть тестовые файлы
              </label>
            </div>
          )}
        </div>
      </div>

      {mode === 'list' ? (
        <div className="flex-1 overflow-auto px-6 py-4">
          <table className="w-full text-xs">
            <thead className="text-muted-foreground text-left">
              <tr><th className="py-2 font-medium">Файл</th><th className="font-medium">Язык</th><th className="font-medium text-right">~Токенов</th></tr>
            </thead>
            <tbody>
              {nodes.slice(0, 500).map((n) => (
                <tr
                  key={n.path}
                  onClick={() => onSelect(n.path)}
                  onDoubleClick={() => onOpen(n.path)}
                  className={`cursor-pointer border-t border-border hover:bg-muted ${n.path === selected ? 'bg-accent' : ''}`}
                >
                  <td className="py-1.5 font-mono">{n.path}</td>
                  <td className="text-muted-foreground">{n.lang ?? '—'}</td>
                  <td className="text-right text-muted-foreground">{n.tokens_est ?? 0}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="flex-1 relative min-h-0">
          <div className="absolute left-6 top-3 text-[11px] text-muted-foreground pointer-events-none">
            <div className="font-semibold uppercase">{selected && depth > 0 ? 'Локальный граф' : 'Весь проект'}</div>
            <div>
              Узлов: {layout.shown.length} · связей: {layout.kept.length}
              {layout.available > layout.shown.length && ` (главные из ${layout.available})`}
              {totalFiles > nodes.length && ` · в проекте ${totalFiles} файлов`}
            </div>
          </div>
          <label className={`${control} absolute right-6 top-3 z-10 flex items-center gap-3 px-3 py-1.5 text-[11px] text-muted-foreground`}>
            Глубина
            <select className="bg-transparent text-foreground appearance-none pr-4" value={depth} onChange={(e) => setDepth(Number(e.target.value))}>
              <option value={1}>1</option>
              <option value={2}>2</option>
              <option value={3}>3</option>
              <option value={0}>всё</option>
            </select>
            <ChevronDown className="w-3 h-3 -ml-4 pointer-events-none" />
          </label>

          {nodes.length === 0 && (
            <div className="absolute inset-0 flex items-center justify-center text-sm text-muted-foreground">
              Индекс пуст — нажмите «Обновить» в карточке индекса
            </div>
          )}

          <svg
            ref={svgRef}
            className="absolute inset-0 w-full h-full cursor-grab select-none"
            viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
            onWheel={(e) => zoom(e.deltaY < 0 ? 1.1 : 1 / 1.1)}
            onMouseDown={(e) => (drag.current = { x: e.clientX, y: e.clientY })}
            onMouseUp={() => (drag.current = null)}
            onMouseLeave={() => (drag.current = null)}
            onMouseMove={(e) => {
              const from = drag.current
              const box = svgRef.current?.getBoundingClientRect()
              if (!from || !box) return
              const scale = Math.max(WIDTH / box.width, HEIGHT / box.height)
              const dx = (e.clientX - from.x) * scale
              const dy = (e.clientY - from.y) * scale
              drag.current = { x: e.clientX, y: e.clientY }
              setView((v) => ({ ...v, x: v.x + dx, y: v.y + dy }))
            }}
          >
            <g transform={`translate(${view.x} ${view.y}) scale(${view.k})`}>
              {layout.kept.map(([a, b]) => {
                const p = layout.position.get(a)!
                const q = layout.position.get(b)!
                const on = a === selected || b === selected
                // лёгкий изгиб перпендикулярно отрезку
                const cx = (p.x + q.x) / 2 + (q.y - p.y) * 0.08
                const cy = (p.y + q.y) / 2 - (q.x - p.x) * 0.08
                return (
                  <path
                    key={`${a}\n${b}`}
                    d={`M${p.x} ${p.y} Q${cx} ${cy} ${q.x} ${q.y}`}
                    fill="none"
                    stroke={on ? '#756291' : '#38363f'}
                    strokeWidth={on ? 1.3 : 0.8}
                  />
                )
              })}
              {layout.shown.map((n, i) => {
                const p = layout.position.get(n.path)!
                const isSelected = n.path === selected
                const r = isSelected ? 9 : 3 + Math.min(5, layout.degree.get(n.path) ?? 0)
                const color = isSelected ? '#b89aff' : groups.color(n.path)
                const strong = isSelected || near.has(n.path)
                return (
                  <g
                    key={n.path}
                    data-path={n.path}
                    className="cursor-pointer"
                    onClick={() => onSelect(n.path)}
                    onDoubleClick={() => onOpen(n.path)}
                  >
                    <title>{n.path}</title>
                    <circle cx={p.x} cy={p.y} r={14} fill="transparent" />
                    {isSelected && <circle cx={p.x} cy={p.y} r={24} fill="#b89aff" opacity={0.08} />}
                    {isSelected && <circle cx={p.x} cy={p.y} r={15} fill="#b89aff" opacity={0.12} />}
                    <circle className="node-dot" cx={p.x} cy={p.y} r={r} fill={color} />
                    {(isSelected || labelled(n.path, i)) && (
                      <text
                        x={p.x}
                        y={p.y + r + (isSelected ? 20 : 13)}
                        textAnchor="middle"
                        fontSize={isSelected ? 11 : 9.5}
                        fill={isSelected ? '#e4e3ea' : strong ? '#aaa5b6' : OTHER_COLOR}
                        stroke="#1a191e"
                        strokeWidth={3}
                        paintOrder="stroke"
                      >
                        {fileName(n.path)}
                      </text>
                    )}
                  </g>
                )
              })}
            </g>
          </svg>

          <div className={`${control} absolute left-6 bottom-3 flex gap-5 px-3 py-1.5 text-[11px] text-muted-foreground`}>
            {groups.top.map((g, i) => (
              <span key={g} className="flex items-center gap-1.5">
                <span className="w-1.5 h-1.5 rounded-full" style={{ background: ['#a691cd', '#85afcf', '#88b9a2', '#c6b080'][i] }} />
                {g}
              </span>
            ))}
          </div>
          <div className={`${control} absolute right-6 bottom-3 flex items-center text-[11px] text-muted-foreground`}>
            <button className="p-2" onClick={() => zoom(1 / 1.2)} title="Отдалить"><Minus className="w-3.5 h-3.5" /></button>
            <span className="w-12 text-center">{Math.round(view.k * 100)}%</span>
            <button className="p-2" onClick={() => zoom(1.2)} title="Приблизить"><Plus className="w-3.5 h-3.5" /></button>
            <button className="p-2 border-l border-border" onClick={() => setView({ x: 0, y: 0, k: 1 })} title="Показать целиком">
              <Maximize className="w-3.5 h-3.5" />
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
