import { useState, useEffect, useCallback } from 'react'
import { RefreshCw } from 'lucide-react'
import type { IndexStatus } from '../types'

/** Журнал вызовов нейросети. Показывает только то, что реально записано в call_log. */
export default function TokenPanel({ projectPath }: { projectPath: string }) {
  const [status, setStatus] = useState<IndexStatus | null>(null)
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(async () => {
    if (!projectPath || !window.atlasAPI) return
    try {
      setStatus(await window.atlasAPI.status(projectPath))
      setError(null)
    } catch (err: any) {
      setError(err.message)
    }
  }, [projectPath])

  useEffect(() => {
    setStatus(null)
    load()
  }, [load])

  const byTool = status?.calls.by_tool ?? []
  const calls = byTool.reduce((sum, t) => sum + t.count, 0)
  const tokens = byTool.reduce((sum, t) => sum + t.tokens_out, 0)

  return (
    <div className="border-t p-3 flex flex-col h-80 overflow-hidden text-[10px]">
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-xs font-semibold">Вызовы нейросети</h3>
        <button onClick={load} className="p-1 hover:bg-muted rounded text-muted-foreground" title="Обновить">
          <RefreshCw className="w-3 h-3" />
        </button>
      </div>
      {error && <div className="text-red-600 mb-2">{error}</div>}
      <div className="text-muted-foreground mb-2">
        За всё время: {calls} вызовов, отдано ~{tokens} токенов
      </div>
      <div className="flex-1 overflow-y-auto space-y-1 font-mono">
        {byTool.length === 0 && <div className="text-muted-foreground">Вызовов через MCP ещё не было</div>}
        {byTool.map((t) => (
          <div key={t.tool} className="flex justify-between">
            <span>{t.tool} × {t.count}</span>
            <span>~{t.tokens_out}</span>
          </div>
        ))}
        {(status?.calls.recent.length ?? 0) > 0 && <div className="font-semibold pt-2">Последние (UTC)</div>}
        {status?.calls.recent.slice(0, 8).map((c, i) => (
          <div key={i} className="flex justify-between text-muted-foreground">
            <span>{c.time} {c.tool}</span>
            <span>~{c.tokens_out}</span>
          </div>
        ))}
      </div>
    </div>
  )
}
