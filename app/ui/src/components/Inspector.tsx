import { useEffect, useMemo, useState } from 'react'
import { ArrowUpRight, PanelRightClose, Plus } from 'lucide-react'
import type { EdgeInfo, GraphEdge, GraphNode, SymbolNode } from '../types'
import { badge, fileLinks, fileName, formatNumber } from '../lib'

interface InspectorProps {
  projectPath: string
  /** путь файла или `путь::имя` */
  target: string | null
  nodes: GraphNode[]
  edges: GraphEdge[]
  version: number
  onSelect: (target: string) => void
  onOpen: (target: string) => void
  onClose: () => void
}

const LETTERS: Record<string, string> = { function: 'ƒ', method: 'm', class: 'C', interface: 'I', type: 'T', constant: 'K', field: 'f' }

interface Details {
  symbols: SymbolNode[]
  lines: number
  incoming: EdgeInfo[]
  outgoing: EdgeInfo[]
}

export default function Inspector(props: InspectorProps) {
  const { projectPath, target, nodes, edges, version, onSelect, onOpen, onClose } = props
  const [tab, setTab] = useState<'overview' | 'links'>('overview')
  const [details, setDetails] = useState<Details | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [showAll, setShowAll] = useState(false)
  const [added, setAdded] = useState(false)

  const path = target?.split('::')[0] ?? null
  const node = nodes.find((n) => n.path === path)
  const links = useMemo(() => (path ? fileLinks(path, nodes, edges.filter((e) => e.confidence !== 'guess')) : null), [path, nodes, edges])

  useEffect(() => {
    setDetails(null)
    setError(null)
    setShowAll(false)
    setAdded(false)
    if (!path || !target) return
    let stale = false
    Promise.all([
      window.atlasAPI.outline({ projectPath, path }),
      window.atlasAPI.read({ projectPath, target: path, mode: 'signature' }),
      window.atlasAPI.neighbors({ projectPath, target }),
    ])
      .then(([outline, read, near]) => {
        if (!stale) setDetails({ symbols: outline.symbols, lines: read.total, incoming: near.incoming, outgoing: near.outgoing })
      })
      .catch((err) => !stale && setError(err.message))
    return () => {
      stale = true
    }
  }, [projectPath, path, target, version])

  /** Сигнатуры модуля и того, что он импортирует, — в буфер обмена. */
  const addToContext = async () => {
    if (!path || !links) return
    try {
      const parts = await Promise.all(
        [path, ...links.imports].map(async (p) => {
          const file = await window.atlasAPI.read({ projectPath, target: p, mode: 'signature' })
          return `## ${p}\n${file.lines.map((l) => `${l.line}: ${l.text}`).join('\n')}`
        }),
      )
      await navigator.clipboard.writeText(parts.join('\n\n'))
      setAdded(true)
    } catch (err: any) {
      setError(err.message)
    }
  }

  const heading = 'flex justify-between text-[11px] font-semibold uppercase text-muted-foreground mb-2'
  const fileRow = (p: string) => (
    <button key={p} onClick={() => onSelect(p)} className="w-full flex items-center gap-3 py-1.5 text-xs text-left hover:text-foreground text-muted-foreground" title={p}>
      <span className="w-6 text-[10px] font-semibold">{badge(p)}</span>
      <span className="flex-1 truncate">{fileName(p)}</span>
      <ArrowUpRight className="w-3 h-3" />
    </button>
  )

  return (
    <aside className="w-[300px] min-[1400px]:w-[376px] shrink-0 bg-panel border-l border-border flex flex-col min-h-0">
      <div className="h-10 shrink-0 border-b border-border flex items-end gap-8 px-6 text-xs">
        {(['overview', 'links'] as const).map((t) => (
          <button key={t} onClick={() => setTab(t)} className={`pb-2.5 border-b-2 ${tab === t ? 'border-primary' : 'border-transparent text-muted-foreground'}`}>
            {t === 'overview' ? 'Обзор' : `Связи ${links?.total ?? ''}`}
          </button>
        ))}
        <button onClick={onClose} className="ml-auto pb-2.5 text-muted-foreground" title="Скрыть инспектор (Ctrl+Shift+B)">
          <PanelRightClose className="w-4 h-4" />
        </button>
      </div>

      {!path ? (
        <div className="p-6 text-xs text-muted-foreground">Выберите файл в обозревателе или узел на графе.</div>
      ) : (
        <>
          <div className="flex-1 overflow-y-auto px-6 py-5 space-y-5">
            <div className="flex gap-3">
              <div className="w-9 h-9 shrink-0 rounded-md bg-muted flex items-center justify-center text-[11px] font-semibold text-blue-600">{badge(path)}</div>
              <div className="min-w-0">
                <div className="text-lg font-semibold truncate">{fileName(path)}</div>
                <div className="text-[11px] text-muted-foreground break-all">{path.split('/').join(' / ')}</div>
              </div>
            </div>
            <div className="flex items-center gap-3 text-[11px]">
              <span className="bg-accent text-accent-foreground rounded px-2 py-0.5">{node?.lang ?? 'файл'}</span>
              <span className="text-muted-foreground">~{formatNumber(node?.tokens_est ?? 0)} токенов</span>
              <button className="ml-auto text-accent-foreground" onClick={() => onOpen(target!)}>Открыть код</button>
            </div>
            {error && <div className="text-xs text-red-700">{error}</div>}

            {tab === 'overview' ? (
              <>
                <div className="border-t border-border pt-4">
                  <div className={heading}>Назначение</div>
                  <p className="text-xs leading-5 whitespace-pre-line">
                    {node?.doc || <span className="text-muted-foreground">В коде нет описания этого модуля.</span>}
                  </p>
                </div>

                <div className="grid grid-cols-3 bg-muted rounded-lg py-3">
                  {[
                    [details?.lines, 'строк'],
                    [links?.total, 'связанных файлов'],
                    [details?.symbols.length, 'символов'],
                  ].map(([value, label]) => (
                    <div key={label} className="px-3">
                      <div className="text-lg">{value ?? '…'}</div>
                      <div className="text-[10px] text-muted-foreground">{label}</div>
                    </div>
                  ))}
                </div>

                <div>
                  <div className={heading}><span>Импортирует</span><span>{links?.imports.length}</span></div>
                  {links?.imports.slice(0, showAll ? undefined : 3).map(fileRow)}
                  {links?.imports.length === 0 && <div className="text-xs text-muted-foreground">Импортов файлов проекта нет</div>}
                  {!showAll && (links?.imports.length ?? 0) > 3 && (
                    <button className="text-[11px] text-accent-foreground pl-9" onClick={() => setShowAll(true)}>
                      Показать ещё {links!.imports.length - 3}
                    </button>
                  )}
                </div>

                <div>
                  <div className={heading}><span>Используется в</span><span>{links?.usedBy.length}</span></div>
                  {links?.usedBy.slice(0, 8).map(fileRow)}
                  {links?.usedBy.length === 0 && <div className="text-xs text-muted-foreground">Статических ссылок не найдено</div>}
                </div>

                <div className="border-t border-border pt-4">
                  <div className={heading}><span>Символы</span><span>{details?.symbols.length}</span></div>
                  {details?.symbols.slice(0, 30).map((s) => (
                    <button key={s.id} onClick={() => onOpen(`${path}::${s.name}`)} className="w-full flex items-center gap-3 py-1 text-left font-mono text-[11px]" title={s.signature ?? s.name}>
                      <span className="w-5 h-5 rounded bg-muted flex items-center justify-center text-accent-foreground">{LETTERS[s.kind] ?? '•'}</span>
                      <span className="truncate">{s.name}{s.kind === 'function' || s.kind === 'method' ? '()' : ''}</span>
                    </button>
                  ))}
                </div>
              </>
            ) : (
              <>
                {([['Исходящие', details?.outgoing], ['Входящие', details?.incoming]] as const).map(([title, list]) => (
                  <div key={title} className="border-t border-border pt-4">
                    <div className={heading}><span>{title}</span><span>{list?.length}</span></div>
                    {list?.slice(0, 40).map((e, i) => (
                      <button key={i} onClick={() => onSelect(`${e.target_path}::${e.target_name}`)} className="w-full flex justify-between gap-2 py-1 text-xs text-left" title={e.target_path}>
                        <span className="truncate">{e.target_name}</span>
                        <span className="text-[10px] text-muted-foreground shrink-0">{e.kind} · {e.confidence}</span>
                      </button>
                    ))}
                    {list?.length === 0 && <div className="text-xs text-muted-foreground">Не найдено (динамические вызовы статически не определяются)</div>}
                  </div>
                ))}
              </>
            )}
          </div>
          <div className="px-6 py-3">
            <button onClick={addToContext} className="w-full bg-primary text-primary-foreground rounded-md py-2.5 text-xs font-medium flex items-center justify-center gap-2">
              <Plus className="w-4 h-4" /> {added ? 'Скопировано в буфер' : 'Добавить в контекст'}
            </button>
            <div className="text-center text-[10px] text-muted-foreground mt-2">Копирует сигнатуры модуля и его прямых импортов</div>
          </div>
        </>
      )}
    </aside>
  )
}
