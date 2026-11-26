import { useState, useEffect, useCallback, useRef } from 'react'
import { Download, Copy, X, RotateCcw } from 'lucide-react'

interface ReportExportProps {
  projectPath: string
  isOpen: boolean
  onClose: () => void
  initialFormat: Format
}

type Format = 'json' | 'markdown' | 'html'

/** Отчёт строит ядро (`atlas report`): интерфейс только показывает и сохраняет. */
export default function ReportExport({ projectPath, isOpen, onClose, initialFormat }: ReportExportProps) {
  const [format, setFormat] = useState<Format>(initialFormat)
  const request = useRef(0)
  const [content, setContent] = useState('')
  const [generating, setGenerating] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const generate = useCallback(async () => {
    if (!projectPath || !window.atlasAPI) return
    const current = ++request.current
    setGenerating(true)
    setError(null)
    setContent('')
    try {
      const text =
        format === 'json'
          ? JSON.stringify(await window.atlasAPI.report({ projectPath, format }), null, 2)
          : await window.atlasAPI.report({ projectPath, format })
      if (current === request.current) setContent(text)
    } catch (err: any) {
      setError(err.message || 'Ошибка генерации отчёта')
    } finally {
      setGenerating(false)
    }
  }, [projectPath, format])

  useEffect(() => {
    if (isOpen) setFormat(initialFormat)
  }, [isOpen, initialFormat])

  useEffect(() => {
    if (isOpen) generate()
  }, [isOpen, generate])

  const download = () => {
    const url = URL.createObjectURL(new Blob([content]))
    const a = document.createElement('a')
    a.href = url
    const ext = format === 'json' ? 'json' : format === 'html' ? 'html' : 'md'
    a.download = `atlas-report-${projectPath.split(/[\\/]/).pop()}.${ext}`
    a.click()
    URL.revokeObjectURL(url)
  }

  if (!isOpen) return null

  const button = 'px-3 py-1.5 text-xs border rounded hover:bg-muted disabled:opacity-50 flex items-center gap-1'

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30">
      <div className="w-full max-w-4xl max-h-[85vh] bg-background border shadow-xl rounded-lg flex flex-col">
        <div className="flex items-center justify-between p-4 border-b gap-3">
          <h2 className="text-lg font-semibold">Экспорт отчёта проекта</h2>
          <div className="flex items-center gap-2">
            {(['markdown', 'json', 'html'] as Format[]).map((f) => (
              <button key={f} onClick={() => setFormat(f)} className={`${button} ${format === f ? 'bg-primary text-primary-foreground' : ''}`}>
                {f === 'json' ? 'JSON' : f === 'html' ? 'HTML' : 'Markdown'}
              </button>
            ))}
            <button onClick={generate} disabled={generating} className={button}>
              <RotateCcw className="w-3 h-3" /> Обновить
            </button>
            <button onClick={() => navigator.clipboard.writeText(content)} disabled={!content} className={button}>
              <Copy className="w-3 h-3" /> Копировать
            </button>
            <button onClick={download} disabled={!content} className={button}>
              <Download className="w-3 h-3" /> Скачать
            </button>
            <button onClick={onClose} className="p-2 rounded hover:bg-muted" title="Закрыть">
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>
        {error && <div className="p-4 text-sm text-red-600">{error}</div>}
        <pre className="flex-1 overflow-auto p-4 text-xs whitespace-pre-wrap">
          {generating ? 'Генерация отчёта…' : content}
        </pre>
      </div>
    </div>
  )
}
