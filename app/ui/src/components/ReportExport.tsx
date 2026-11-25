import { useState, useEffect, useCallback } from 'react'
import {
  FileText, Download, Copy, X, Loader2, AlertTriangle, Share2
} from 'lucide-react'
import type { ProjectOverview, GraphNode } from '../types'

interface ReportExportProps {
  projectPath: string
  overview: ProjectOverview | null
  graphNodes: GraphNode[]
  isOpen: boolean
  onClose: () => void
}

interface ReportData {
  project: {
    root: string
    generated_at: string
    files: number
    symbols: number
    languages: Record<string, number>
  }
  index_stats: {
    nodes_by_kind: Record<string, number>
    edges_by_kind: Record<string, number>
    total_nodes: number
    total_edges: number
  }
  top_symbols: Array<{
    name: string
    kind: string
    path: string
    rank: number
    tokens_est: number
    line_start?: number
    line_end?: number
  }>
  call_graph: {
    nodes: Array<{ id: string; name: string; kind: string; path: string; rank: number }>
    edges: Array<{ source: string; target: string; kind: string; confidence: string }>
  }
  token_usage: {
    session: { total_in: number; total_out: number; calls: number; savings_pct: number }
    day: { total_in: number; total_out: number; calls: number; savings_pct: number }
    week: { total_in: number; total_out: number; calls: number; savings_pct: number }
  }
  settings: {
    exclusions: string[]
    budgets: Record<string, number>
  }
}

export default function ReportExport({
  projectPath,
  overview,
  graphNodes,
  isOpen,
  onClose,
}: ReportExportProps) {
  const [format, setFormat] = useState<'json' | 'markdown'>('json')
  const [generating, setGenerating] = useState(false)
  const [report, setReport] = useState<ReportData | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)

  const generateReport = useCallback(async () => {
    if (!projectPath || !window.atlasAPI || !overview) return
    setGenerating(true)
    setError(null)
    try {
      // Пытаемся получить отчёт через MCP report tool
      const result = await window.atlasAPI.report(projectPath, { format: 'json' })
      if (result.ok && result.report) {
        setReport(result.report)
      } else {
        // Fallback: строим отчёт локально из доступных данных
        const localReport = await buildLocalReport()
        setReport(localReport)
      }
    } catch (err: any) {
      // Fallback: строим отчёт локально
      try {
        const localReport = await buildLocalReport()
        setReport(localReport)
      } catch {
        setError(err.message || 'Ошибка генерации отчёта')
      }
    } finally {
      setGenerating(false)
    }
  }, [projectPath, overview, graphNodes])

  const buildLocalReport = async (): Promise<ReportData> => {
    if (!window.atlasAPI) throw new Error('API недоступен')

    // Получаем статус для статистик
    const status = await window.atlasAPI.status(projectPath)
    const callLog = await window.atlasAPI.callLog(projectPath, {})
    const settingsAll = await window.atlasAPI.settings.all(projectPath)

    // Получаем рёбра графа через neighbors для топ-узлов
    const edges: Array<{ source: string; target: string; kind: string; confidence: string }> = []
    for (const node of graphNodes.slice(0, 50)) {
      try {
        const neighbors = await window.atlasAPI.neighbors({ projectPath, target: node.id })
        for (const out of neighbors.outgoing.slice(0, 10)) {
          edges.push({
            source: node.id,
            target: out.target_id,
            kind: out.kind,
            confidence: out.confidence,
          })
        }
      } catch {
        // игнорируем ошибки отдельных узлов
      }
    }

    return {
      project: {
        root: projectPath,
        generated_at: new Date().toISOString(),
        files: overview.files,
        symbols: overview.top.length,
        languages: overview.langs,
      },
      index_stats: {
        nodes_by_kind: {},
        edges_by_kind: {},
        total_nodes: overview.top.length,
        total_edges: edges.length,
      },
      top_symbols: overview.top.map(n => ({
        name: n.name,
        kind: n.kind,
        path: n.path,
        rank: n.rank || 0,
        tokens_est: n.tokens_est || 0,
      })),
      call_graph: {
        nodes: graphNodes.map(n => ({
          id: n.id,
          name: n.name,
          kind: n.kind,
          path: n.path,
          rank: n.rank || 0,
        })),
        edges,
      },
      token_usage: {
        session: { total_in: 0, total_out: 0, calls: 0, savings_pct: 0 },
        day: { total_in: 0, total_out: 0, calls: 0, savings_pct: 0 },
        week: { total_in: 0, total_out: 0, calls: 0, savings_pct: 0 },
      },
      settings: {
        exclusions: settingsAll.settings?.exclusions || [],
        budgets: settingsAll.settings?.budgets || {},
      },
    }
  }

  const downloadReport = () => {
    if (!report) return
    const content = format === 'json'
      ? JSON.stringify(report, null, 2)
      : renderMarkdown(report)
    const blob = new Blob([content], { type: format === 'json' ? 'application/json' : 'text/markdown' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `atlas-report-${projectPath.split('/').pop()}-${new Date().toISOString().split('T')[0]}.${format === 'json' ? 'json' : 'md'}`
    a.click()
    URL.revokeObjectURL(url)
  }

  const copyReport = () => {
    if (!report) return
    const content = format === 'json'
      ? JSON.stringify(report, null, 2)
      : renderMarkdown(report)
    navigator.clipboard.writeText(content)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  const shareReport = async () => {
    if (!report) return
    const content = renderMarkdown(report)
    if (navigator.share) {
      try {
        await navigator.share({
          title: `Atlas Report - ${projectPath.split('/').pop()}`,
          text: content,
        })
      } catch {
        // пользователь отменил
      }
    } else {
      copyReport()
    }
  }

  // Автогенерация при открытии
  useEffect(() => {
    if (isOpen && !report && !generating) {
      generateReport()
    }
  }, [isOpen, generateReport, report, generating])

  if (!isOpen) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center pointer-events-none" onClick={onClose}>
      <div className="pointer-events-auto w-full max-w-4xl max-h-[85vh] bg-background border shadow-xl rounded-lg overflow-hidden flex flex-col animate-slide-down">
        {/* Заголовок */}
        <div className="flex items-center justify-between p-4 border-b bg-muted/50">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded bg-green-500 flex items-center justify-center">
              <FileText className="w-4 h-4 text-white" />
            </div>
            <div>
              <h2 className="text-lg font-semibold">Экспорт отчёта проекта</h2>
              <p className="text-xs text-muted-foreground">{projectPath}</p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <button onClick={shareReport} className="px-3 py-1.5 text-xs border rounded hover:bg-muted text-sm flex items-center gap-1">
              <Share2 className="w-3 h-3" /> Поделиться
            </button>
            <button onClick={copyReport} disabled={!report || generating} className="px-3 py-1.5 text-xs border rounded hover:bg-muted disabled:opacity-50 flex items-center gap-1">
              <Copy className="w-3 h-3" /> {copied ? 'Скопировано!' : 'Копировать'}
            </button>
            <button onClick={downloadReport} disabled={!report || generating} className="px-3 py-1.5 text-xs bg-primary text-primary-foreground rounded hover:bg-primary/90 disabled:opacity-50 flex items-center gap-1">
              <Download className="w-3 h-3" /> {generating ? <Loader2 className="w-3 h-3 animate-spin" /> : 'Скачать'}
            </button>
            <button onClick={onClose} className="p-2 rounded hover:bg-muted text-muted-foreground">
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Формат и действия */}
        <div className="border-b p-4 bg-muted/30 flex items-center justify-between gap-4 flex-wrap">
          <div className="flex items-center gap-4">
            <span className="text-sm font-medium">Формат:</span>
            <div className="flex border rounded bg-background" role="group">
              <button
                onClick={() => setFormat('json')}
                className={`px-4 py-2 text-sm border-r transition-colors ${format === 'json' ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-muted'}`}
              >
                JSON
              </button>
              <button
                onClick={() => setFormat('markdown')}
                className={`px-4 py-2 text-sm transition-colors ${format === 'markdown' ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-muted'}`}
              >
                Markdown
              </button>
            </div>
          </div>
          <button
            onClick={generateReport}
            disabled={generating}
            className="px-4 py-2 text-sm border rounded hover:bg-muted disabled:opacity-50 flex items-center gap-1"
          >
            {generating ? (
              <>
                <Loader2 className="w-3 h-3 animate-spin" /> Генерирование…
              </>
            ) : (
              <>
                <RotateCcw className="w-3 h-3" /> Перегенерировать
              </>
            )}
          </button>
        </div>

        {error && (
          <div className="p-4 bg-red-500/10 border-b border-red-500/20 text-red-700 text-sm flex items-center gap-2">
            <AlertTriangle className="w-4 h-4" />
            {error}
          </div>
        )}

        {/* Превью отчёта */}
        <div className="flex-1 overflow-auto p-4">
          {generating && !report && (
            <div className="flex items-center justify-center h-64 text-muted-foreground">
              <Loader2 className="w-8 h-8 animate-spin mr-2" />
              Генерация отчёта…
            </div>
          )}

          {report && (
            <div className="font-mono text-sm bg-muted/30 rounded p-4 max-h-[50vh] overflow-auto">
              {format === 'json' ? (
                <pre className="whitespace-pre-wrap text-foreground">{JSON.stringify(report, null, 2)}</pre>
              ) : (
                <pre className="whitespace-pre-wrap text-foreground">{renderMarkdown(report)}</pre>
              )}
            </div>
          )}

          {!report && !generating && (
            <div className="text-center py-12 text-muted-foreground">
              <FileText className="w-12 h-12 mx-auto mb-4 opacity-50" />
              <p>Нажмите «Перегенерировать» для создания отчёта</p>
            </div>
          )}
        </div>

        {/* Статистика снизу */}
        {report && (
          <div className="border-t p-3 bg-muted/30 grid grid-cols-4 gap-4 text-xs text-center">
            <div className="p-2 bg-background rounded">
              <div className="font-mono text-lg">{report.project.files}</div>
              <div className="text-muted-foreground">Файлов</div>
            </div>
            <div className="p-2 bg-background rounded">
              <div className="font-mono text-lg">{report.project.symbols}</div>
              <div className="text-muted-foreground">Символов</div>
            </div>
            <div className="p-2 bg-background rounded">
              <div className="font-mono text-lg">{report.call_graph.edges.length}</div>
              <div className="text-muted-foreground">Связей</div>
            </div>
            <div className="p-2 bg-background rounded">
              <div className="font-mono text-lg">{Object.keys(report.project.languages).length}</div>
              <div className="text-muted-foreground">Языков</div>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

function renderMarkdown(report: ReportData): string {
  const lines: string[] = []
  lines.push(`# Atlas Project Report`)
  lines.push(``)
  lines.push(`**Проект:** ${report.project.root}`)
  lines.push(`**Сгенерирован:** ${new Date(report.project.generated_at).toLocaleString('ru-RU')}`)
  lines.push(``)

  lines.push(`## 📊 Общая статистика`)
  lines.push(``)
  lines.push(`| Метрика | Значение |`)
  lines.push(`|---------|----------|`)
  lines.push(`| Файлов | ${report.project.files} |`)
  lines.push(`| Символов | ${report.project.symbols} |`)
  lines.push(`| Языков | ${Object.entries(report.project.languages).map(([l, n]) => `${l}: ${n}`).join(', ')} |`)
  lines.push(`| Узлов в индексе | ${report.index_stats.total_nodes} |`)
  lines.push(`| Связей в графе | ${report.index_stats.total_edges} |`)
  lines.push(``)

  lines.push(`## 🗣️ Языки проекта`)
  lines.push(``)
  for (const [lang, count] of Object.entries(report.project.languages)) {
    lines.push(`- **${lang}**: ${count} файлов`)
  }
  lines.push(``)

  lines.push(`## 🏆 Топ символов (по PageRank)`)
  lines.push(``)
  lines.push(`| # | Символ | Тип | Файл | Ранг | Токенов |`)
  lines.push(`|---|--------|-----|------|------|---------|`)
  report.top_symbols.slice(0, 20).forEach((s, i) => {
    lines.push(`| ${i + 1} | \`${s.name}\` | ${s.kind} | ${s.path} | ${s.rank.toFixed(4)} | ${s.tokens_est} |`)
  })
  lines.push(``)

  lines.push(`## 🔗 Граф вызовов (топ-${report.call_graph.nodes.length} узлов)`)
  lines.push(``)
  lines.push(`### Узлы`)
  lines.push(``)
  lines.push(`| ID | Имя | Тип | Файл | Ранг |`)
  lines.push(`|----|-----|-----|------|------|`)
  report.call_graph.nodes.forEach(n => {
    lines.push(`| ${n.id.slice(0, 8)} | \`${n.name}\` | ${n.kind} | ${n.path} | ${n.rank.toFixed(4)} |`)
  })
  lines.push(``)

  lines.push(`### Рёбра (${report.call_graph.edges.length})`)
  lines.push(``)
  lines.push(`| Источник | Цель | Тип | Уверенность |`)
  lines.push(`|----------|------|-----|-------------|`)
  report.call_graph.edges.slice(0, 50).forEach(e => {
    lines.push(`| ${e.source.slice(0, 8)} | ${e.target.slice(0, 8)} | ${e.kind} | ${e.confidence} |`)
  })
  if (report.call_graph.edges.length > 50) {
    lines.push(`| … | … | … | … |`)
    lines.push(`_Показано 50 из ${report.call_graph.edges.length} рёбер_`)
  }
  lines.push(``)

  lines.push(`## 💰 Расход токенов`)
  lines.push(``)
  lines.push(`| Период | Вход | Выход | Вызовы | Экономия |`)
  lines.push(`|--------|------|-------|--------|----------|`)
  for (const [period, data] of Object.entries(report.token_usage)) {
    lines.push(`| ${period} | ${data.total_in} | ${data.total_out} | ${data.calls} | ${data.savings_pct}% |`)
  }
  lines.push(``)

  lines.push(`## ⚙️ Настройки`)
  lines.push(``)
  lines.push(`### Исключения (${report.settings.exclusions.length})`)
  lines.push(``)
  report.settings.exclusions.forEach(p => lines.push(`- \`${p}\``))
  lines.push(``)

  lines.push(`### Бюджеты токенов`)
  lines.push(``)
  lines.push(`| Инструмент | Бюджет (токенов) |`)
  lines.push(`|------------|------------------|`)
  for (const [tool, budget] of Object.entries(report.settings.budgets)) {
    lines.push(`| ${tool} | ${budget} |`)
  }
  lines.push(``)

  lines.push(`---`)
  lines.push(`*Отчёт сгенерирован Atlas Navigator*`)
  return lines.join('\n')
}