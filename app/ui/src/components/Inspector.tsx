import { useState, useEffect, useCallback } from 'react'
import {
  File,
  ChevronRight,
  ChevronDown,
  Link,
  Clock,
  ArrowUpRight,
  ArrowLeftRight,
} from 'lucide-react'

interface InspectorProps {
  projectPath: string
  nodeId: string | null
  onGraphSelect: (nodeId: string) => void
}

export default function Inspector({ projectPath, nodeId, onGraphSelect }: InspectorProps) {
  const [expandedSections, setExpandedSections] = useState<Set<string>>(new Set(['properties']))
  const [details, setDetails] = useState<{
    name: string
    kind: string
    path: string
    line_start?: number
    line_end?: number
    signature?: string
    doc?: string
  } | null>(null)
  const [edges, setEdges] = useState<{ outgoing: any[]; incoming: any[] }>({
    outgoing: [],
    incoming: [],
  })
  const [loading, setLoading] = useState(false)

  const toggleSection = (section: string) => {
    setExpandedSections((prev) => {
      const next = new Set(prev)
      if (next.has(section)) next.delete(section)
      else next.add(section)
      return next
    })
  }

  const isExpanded = (section: string) => expandedSections.has(section)

  const loadNode = useCallback(async (id: string) => {
    if (!window.atlasAPI || !id) return
    setLoading(true)
    try {
      // Пытаемся найти узел через соседей (symbol -> neighbours)
      const result = await window.atlasAPI.neighbors({ projectPath, target: id })
      setDetails({
        name: result.node.name,
        kind: result.node.kind,
        path: result.node.path,
      })
      setEdges({
        outgoing: result.outgoing,
        incoming: result.incoming,
      })
    } catch (_) {
      // Если не символ, попробуем как путь к файлу
      setDetails({ name: id.split('/').pop() ?? id, kind: 'file', path: id })
      setEdges({ outgoing: [], incoming: [] })
    } finally {
      setLoading(false)
    }
  }, [projectPath])

  useEffect(() => {
    if (nodeId) loadNode(nodeId)
  }, [nodeId, loadNode])

  if (!nodeId) {
    return (
      <div className="h-full flex flex-col p-4">
        <div className="flex items-center gap-3 text-muted-foreground">
          <File className="w-5 h-5 opacity-50" />
          <span>Выберите узел для просмотра деталей</span>
        </div>
        <div className="mt-6 p-4 border rounded bg-muted/30">
          <h4 className="text-xs font-semibold mb-2">Доступные действия:</h4>
          <ul className="text-xs space-y-1">
            <li>• Выберите файл в дереве слева</li>
            <li>• Кликните по узлу на графе</li>
            <li>• Используйте поиск (Ctrl+P)</li>
          </ul>
        </div>
      </div>
    )
  }

  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* Заголовок */}
      <div className="p-3 border-b bg-muted/30">
        <h3 className="text-sm font-semibold">{details?.name || nodeId}</h3>
        <p className="text-[10px] text-muted-foreground">
          {details?.kind || 'symbol'} • {details?.path || nodeId}
        </p>
      </div>

      <div className="flex-1 overflow-y-auto p-3 space-y-4">
        {/* Свойства */}
        <div className="border rounded p-3">
          <div className="flex items-center gap-2 mb-3 cursor-pointer" onClick={() => toggleSection('properties')}>
            {isExpanded('properties') ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
            <span className="font-semibold text-xs">Свойства</span>
          </div>
          {isExpanded('properties') && (
            <div className="space-y-1 text-xs">
              <div className="flex justify-between">
                <span className="text-muted-foreground">Путь:</span>
                <span className="font-mono truncate" title={details?.path || nodeId}>
                  {details?.path || nodeId}
                </span>
              </div>
              {details?.line_start !== undefined && details?.line_end !== undefined && (
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Строки:</span>
                  <span>{details.line_start}-{details.line_end}</span>
                </div>
              )}
              {details?.signature && (
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Сигнатура:</span>
                  <span className="font-mono truncate" title={details.signature}>
                    {details.signature}
                  </span>
                </div>
              )}
              <div className="flex justify-between">
                <span className="text-muted-foreground">Тип:</span>
                <span>{details?.kind || '—'}</span>
              </div>
            </div>
          )}
        </div>

        {/* Связи */}
        <div className="border rounded p-3">
          <div className="flex items-center gap-2 mb-3 cursor-pointer" onClick={() => toggleSection('edges')}>
            {isExpanded('edges') ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
            <span className="font-semibold text-xs">Связи</span>
          </div>
          {isExpanded('edges') && (
            <div className="space-y-2 text-xs">
              {edges.outgoing.length > 0 && (
                <div className="space-y-1">
                  <div className="flex items-center gap-2 mb-1">
                    <ArrowUpRight className="w-3 h-3 text-green-600" />
                    <span className="text-green-700 font-semibold">Вызывает ({edges.outgoing.length}):</span>
                  </div>
                  {edges.outgoing.slice(0, 10).map((e) => (
                    <div
                      key={e.target_id}
                      className="flex items-center justify-between px-2 py-1 bg-green-50 rounded hover:bg-green-100 cursor-pointer group"
                      onClick={() => onGraphSelect(e.target_id)}
                    >
                      <span className="truncate">{e.target_name}</span>
                      <span className="text-[10px] text-green-600">{e.kind}</span>
                    </div>
                  ))}
                  {edges.outgoing.length > 10 && (
                    <div className="text-[10px] text-muted-foreground text-center">
                      +{edges.outgoing.length - 10} ещё
                    </div>
                  )}
                </div>
              )}
              {edges.incoming.length > 0 && (
                <div className="mt-2 space-y-1">
                  <div className="flex items-center gap-2 mb-1">
                    <ArrowLeftRight className="w-3 h-3 text-blue-600" />
                    <span className="text-blue-700 font-semibold">Входит в ({edges.incoming.length}):</span>
                  </div>
                  {edges.incoming.slice(0, 10).map((e) => (
                    <div
                      key={e.target_id}
                      className="flex items-center justify-between px-2 py-1 bg-blue-50 rounded hover:bg-blue-100 cursor-pointer group"
                      onClick={() => onGraphSelect(e.target_id)}
                    >
                      <span className="truncate">{e.target_name}</span>
                      <span className="text-[10px] text-blue-600">{e.kind}</span>
                    </div>
                  ))}
                  {edges.incoming.length > 10 && (
                    <div className="text-[10px] text-muted-foreground text-center">
                      +{edges.incoming.length - 10} ещё
                    </div>
                  )}
                </div>
              )}
              {edges.outgoing.length === 0 && edges.incoming.length === 0 && (
                <div className="text-xs text-muted-foreground text-center py-2">
                  Связей не найдено (динамические вызовы статически не определяются)
                </div>
              )}
            </div>
          )}
        </div>

        {loading && (
          <div className="text-center text-xs text-muted-foreground py-4">
            Загрузка связей…
          </div>
        )}
      </div>

      {/* Действия */}
      <div className="border-t p-3 flex gap-2">
        <button
          className="flex-1 px-2 py-1.5 text-xs border rounded hover:bg-muted"
          onClick={() => onGraphSelect(nodeId)}
        >
          На граф
        </button>
        <button className="flex-1 px-2 py-1.5 text-xs border rounded hover:bg-muted text-red-500">
          Закрыть для НС
        </button>
      </div>
    </div>
  )
}
