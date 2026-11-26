import { useCallback, useEffect, useState } from 'react'
import { ArrowUpRight, Copy } from 'lucide-react'
import type { ProjectMap } from '../types'
import { formatNumber } from '../lib'

interface ContextPanelProps {
  projectPath: string
  /** меняется после переиндексации */
  version: number
  onExport: () => void
}

/** «Контекст для нейросети»: ровно тот текст, который возвращает инструмент overview. */
export default function ContextPanel({ projectPath, version, onExport }: ContextPanelProps) {
  const [map, setMap] = useState<ProjectMap | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)

  const load = useCallback(async () => {
    try {
      setMap(await window.atlasAPI.map(projectPath))
      setError(null)
    } catch (err: any) {
      setError(err.message)
    }
  }, [projectPath])

  useEffect(() => {
    setMap(null)
    load()
  }, [load, version])

  const copy = async () => {
    if (!map) return
    await navigator.clipboard.writeText(map.text)
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }

  const saved = map && map.full_tokens > map.tokens ? Math.floor((1 - map.tokens / map.full_tokens) * 1000) / 10 : null

  return (
    <section className="h-[26vh] min-h-[190px] max-h-64 shrink-0 border-t border-border px-6 py-4 flex flex-col">
      <div className="flex items-start justify-between">
        <div>
          <div className="flex items-center gap-4">
            <h2 className="text-base font-semibold">Контекст для нейросети</h2>
            <span className="text-[10px] bg-muted rounded px-2 py-0.5 text-accent-foreground">Markdown</span>
          </div>
          <div className="text-xs text-muted-foreground mt-1">Структура проекта вместо тысяч строк кода</div>
        </div>
        <button
          onClick={copy}
          disabled={!map}
          className="bg-primary text-primary-foreground rounded-md px-4 py-2 text-xs font-medium flex items-center gap-2 disabled:opacity-50"
        >
          <Copy className="w-3.5 h-3.5" /> {copied ? 'Скопировано' : 'Скопировать карту'}
        </button>
      </div>

      <div className="flex-1 min-h-0 flex gap-6 mt-3">
        <div className="w-44 shrink-0 border-r border-border">
          <div className="text-3xl font-semibold">{map ? formatNumber(map.tokens) : '…'}</div>
          <div className="text-[11px] text-muted-foreground">токенов в карте (оценка)</div>
          {saved !== null && (
            <div className="inline-block mt-2 rounded bg-green-50 text-green-600 text-[11px] px-2 py-1">на {saved.toLocaleString('ru-RU')}% меньше кода</div>
          )}
          {map && (
            <div className="text-[11px] text-muted-foreground mt-2">{formatNumber(map.full_tokens)} — все файлы целиком</div>
          )}
        </div>
        <div className="flex-1 min-w-0 overflow-auto bg-[#17161b] rounded-md py-2 font-mono text-[11px] leading-5">
          {error && <div className="px-3 text-red-700">{error}</div>}
          {map?.text.split('\n').map((line, i) => (
            <div key={i} className="flex">
              <span className="w-10 shrink-0 text-right pr-3 text-muted-foreground opacity-50">{String(i + 1).padStart(2, '0')}</span>
              <span className={`whitespace-pre ${line.startsWith('#') ? 'text-accent-foreground' : ''}`}>{line}</span>
            </div>
          ))}
        </div>
      </div>

      <div className="flex justify-end gap-5 pt-2 text-[11px] text-muted-foreground">
        <button onClick={load} className="hover:text-foreground">Обновить карту</button>
        <button onClick={onExport} className="text-accent-foreground flex items-center gap-1">
          Настроить экспорт <ArrowUpRight className="w-3 h-3" />
        </button>
      </div>
    </section>
  )
}
