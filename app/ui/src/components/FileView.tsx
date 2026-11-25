import { useState, useEffect, useRef, useCallback } from 'react'
import {
  EditorView, basicSetup, Decoration, DecorationSet,
  ViewPlugin, ViewUpdate, WidgetType
} from '@codemirror/view'
import { EditorState, StateField, StateEffect } from '@codemirror/state'
import { rust } from '@codemirror/lang-rust'
import { python } from '@codemirror/lang-python'
import { javascript } from '@codemirror/lang-javascript'
import { json } from '@codemirror/lang-json'
import { markdown } from '@codemirror/lang-markdown'
import { toml } from '@codemirror/lang-toml'
import { oneDark } from '@codemirror/theme-one-dark'
import { Copy, Link2, Zap } from 'lucide-react'

interface FileViewProps {
  projectPath: string
  nodeId: string | null
  onGoToDefinition: (symbol: string, path?: string) => void
  onGraphHighlight: (nodeId: string) => void
}

type ViewMode = 'full' | 'folded' | 'signature' | 'doc'

function getLanguageSupport(lang: string) {
  switch (lang) {
    case 'rust':
      return rust()
    case 'python':
      return python()
    case 'typescript':
    case 'tsx':
      return javascript({ jsx: true, typescript: true })
    case 'javascript':
    case 'jsx':
      return javascript({ jsx: true })
    case 'json':
      return json()
    case 'markdown':
      return markdown()
    case 'toml':
      return toml()
    default:
      return []
  }
}

// Эффект для подсветки AI-токенов (имитация)
const aiHighlightPlugin = ViewPlugin.fromClass(class {
  decorations: DecorationSet
  constructor(view: EditorView) {
    this.decorations = this.buildDecorations(view)
  }
  update(update: ViewUpdate) {
    if (update.docChanged || update.viewportChanged) {
      this.decorations = this.buildDecorations(update.view)
    }
  }
  buildDecorations(view: EditorView): DecorationSet {
    const builder = []
    // Подсвечиваем строки, которые были "прочитаны" ИИ (демо)
    for (const { from, to } of view.visibleRanges) {
      for (let pos = from; pos <= to; ) {
        const line = view.state.doc.lineAt(pos)
        // Демо: подсвечиваем каждую 7-ю строку как "ИИ-активную"
        if (line.number % 7 === 0 && line.text.trim()) {
          builder.push(
            Decoration.line({ class: 'cm-ai-highlight' }).range(line.from)
          )
        }
        pos = line.to + 1
      }
    }
    return DecorationSet.from(builder)
  }
}, {
  decorations: v => v.decorations
})

// Виджет для свёрнутых блоков
class FoldWidget extends WidgetType {
  constructor(private lines: number) { super() }
  toDOM() {
    const span = document.createElement('span')
    span.className = 'cm-fold-widget'
    span.textContent = `  … ${this.lines} строк свернуто …`
    return span
  }
  ignoreEvent() { return false }
}

// Состояние для режима просмотра
const viewModeField = StateField.define<ViewMode>({
  create: () => 'full',
  update(mode, tr) {
    for (const effect of tr.effects) {
      if (effect.is(setViewMode)) return effect.value
    }
    return mode
  },
})

const setViewMode = StateEffect.define<ViewMode>()

export default function FileView({
  projectPath,
  nodeId,
  onGoToDefinition,
  onGraphHighlight,
}: FileViewProps) {
  const editorRef = useRef<HTMLDivElement>(null)
  const viewRef = useRef<EditorView | null>(null)
  const [language, setLanguage] = useState<string>('text')
  const [content, setContent] = useState<string>('')
  const [loading, setLoading] = useState(false)
  const [symbols, setSymbols] = useState<Array<{ id: string; name: string; kind: string; line: number; signature?: string }>>([])
  const [viewMode, setViewModeState] = useState<ViewMode>('full')
  const [aiTokensUsed, setAiTokensUsed] = useState(0)
  const [showAiHighlight, setShowAiHighlight] = useState(false)

  // Создаём редактор при монтировании
  useEffect(() => {
    if (!editorRef.current || !window.atlasAPI) return

    const state = EditorState.create({
      doc: content,
      extensions: [
        basicSetup,
        EditorView.editable.of(false),
        EditorView.lineWrapping,
        getLanguageSupport(language),
        oneDark,
        viewModeField,
        aiHighlightPlugin,
        EditorView.theme({
          '&': { height: '100%', fontSize: '13px', lineHeight: '1.6' },
          '.cm-gutters': {
            backgroundColor: 'transparent',
            borderRight: '1px solid #374151',
            minWidth: '3.5rem',
            paddingRight: '0.5rem',
          },
          '.cm-lineNumbers': { color: '#9ca3af' },
          '.cm-ai-highlight': {
            backgroundColor: 'rgba(34, 197, 94, 0.1) !important',
          },
          '.cm-fold-widget': {
            color: '#9ca3af',
            fontStyle: 'italic',
            padding: '0 0.5rem',
            backgroundColor: 'rgba(156, 163, 175, 0.1)',
            borderRadius: '4px',
            marginLeft: '0.5rem',
          },
          '.cm-content': { caretColor: 'transparent' },
        }),
      ],
    })

    const view = new EditorView({ state, parent: editorRef.current })
    viewRef.current = view

    return () => { view.destroy() }
  }, [language, content])

  // Загружаем содержимое файла при изменении nodeId
  useEffect(() => {
    if (!nodeId || !window.atlasAPI || !viewRef.current) return

    const loadFile = async () => {
      setLoading(true)
      try {
        // Определяем режим загрузки
        const mode = viewMode === 'folded' ? 'folded' : viewMode
        const result = await window.atlasAPI.read({
          projectPath,
          path: nodeId,
          mode,
        })
        const text = result.lines.map(l => l.text).join('\n')
        setContent(text)
        const ext = nodeId.split('.').pop() ?? 'text'
        setLanguage(ext)
        viewRef.current?.dispatch({
          changes: { from: 0, to: viewRef.current.state.doc.length, insert: text },
        })

        // Оценка токенов
        setAiTokensUsed(Math.ceil(text.length / 4))
      } catch (err) {
        console.error('Failed to load file:', err)
      } finally {
        setLoading(false)
      }
    }

    loadFile()

    // Загружаем контур файла (символы)
    window.atlasAPI
      .outline({ projectPath, path: nodeId })
      .then((result) => {
        setSymbols(
          result.symbols.map((s) => ({
            id: s.id,
            name: s.name,
            kind: s.kind,
            line: s.line_start ?? 0,
            signature: s.signature,
          }))
        )
      })
      .catch(() => {
        // outline может быть недоступен для неиндексированных файлов
      })
  }, [nodeId, projectPath, viewMode])

  // Обработчик смены режима
  const handleModeChange = useCallback((mode: ViewMode) => {
    setViewModeState(mode)
    if (viewRef.current) {
      viewRef.current.dispatch({ effects: setViewMode.of(mode) })
    }
    // Перезагружаем содержимое в новом режиме
    if (nodeId && window.atlasAPI) {
      window.atlasAPI.read({ projectPath, path: nodeId, mode })
        .then(result => {
          const text = result.lines.map(l => l.text).join('\n')
          setContent(text)
          viewRef.current?.dispatch({
            changes: { from: 0, to: viewRef.current.state.doc.length, insert: text },
          })
          setAiTokensUsed(Math.ceil(text.length / 4))
        })
    }
  }, [nodeId, projectPath])

  // Переход к определению (Ctrl+Click)
  const handleClick = useCallback((e: React.MouseEvent) => {
    if ((e.ctrlKey || e.metaKey) && e.target instanceof HTMLElement) {
      const target = e.target.closest('.cm-line')
      if (target && viewRef.current) {
        const pos = viewRef.current.posAtCoords({ x: e.clientX, y: e.clientY })
        if (pos) {
          const line = viewRef.current.state.doc.lineAt(pos)
          const text = line.text
          // Ищем вызов функции в строке
          const callMatch = text.match(/(\w+)\s*\(/)
          if (callMatch) {
            const funcName = callMatch[1]
            onGoToDefinition(funcName, nodeId || undefined)
          }
        }
      }
    }
  }, [nodeId, onGoToDefinition])

  // Копирование пути
  const copyPath = () => {
    if (nodeId) navigator.clipboard.writeText(nodeId)
  }

  if (!nodeId) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="text-center">
          <FileText className="w-12 h-12 text-muted-foreground mx-auto mb-2" />
          <p className="text-muted-foreground text-sm">Выберите символ или файл для просмотра</p>
        </div>
      </div>
    )
  }

  const fileName = nodeId.split('/').pop() || nodeId
  const isSymbol = nodeId.includes('::')

  return (
    <div className="h-full flex flex-col bg-background">
      {/* Заголовок файла с тулбаром */}
      <div className="h-9 border-b flex items-center justify-between px-3 text-[11px] bg-muted/50">
        <div className="flex items-center gap-2 flex-1 min-w-0">
          <span className="truncate font-medium">{fileName}</span>
          {isSymbol && (
            <>
              <span className="text-muted-foreground">•</span>
              <span className="text-blue-500">символ: {nodeId.split('::').pop()}</span>
            </>
          )}
          <span className="text-muted-foreground">•</span>
          <span className="text-blue-500 text-capitalize">{language}</span>
          {loading && <span className="text-[10px] text-muted-foreground">• загрузка…</span>}
        </div>
        <div className="flex items-center gap-1 ml-4">
          {/* Режим просмотра */}
          <div className="flex border rounded bg-background" role="group" aria-label="Режим просмотра">
            {(['full', 'folded', 'signature', 'doc'] as ViewMode[]).map(mode => (
              <button
                key={mode}
                onClick={() => handleModeChange(mode)}
                className={`px-2 py-1 text-[10px] transition-colors border-r last:border-0 ${
                  viewMode === mode
                    ? 'bg-primary text-primary-foreground'
                    : 'text-muted-foreground hover:bg-muted'
                }`}
                title={mode === 'full' ? 'Полный код' : mode === 'folded' ? 'Свёрнуто (только каркас)' : mode === 'signature' ? 'Только сигнатура' : 'Документация'}
              >
                {mode.charAt(0).toUpperCase()}
              </button>
            ))}
          </div>
          <div className="w-px h-5 bg-border mx-1" />
          {/* AI подсветка */}
          <button
            onClick={() => setShowAiHighlight(v => !v)}
            className={`p-1.5 rounded transition-colors ${showAiHighlight ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-muted'}`}
            title={showAiHighlight ? 'Скрыть AI-подсветку' : 'Показать строки, использованные ИИ'}
          >
            <Zap className="w-3 h-3" />
          </button>
          <button
            onClick={copyPath}
            className="p-1.5 rounded transition-colors text-muted-foreground hover:bg-muted"
            title="Копировать путь"
          >
            <Copy className="w-3 h-3" />
          </button>
          <button
            onClick={() => onGraphHighlight(nodeId)}
            className="p-1.5 rounded transition-colors text-muted-foreground hover:bg-muted"
            title="Подсветить на графе"
          >
            <Link2 className="w-3 h-3" />
          </button>
        </div>
      </div>

      {/* Редактор CodeMirror */}
      <div className="flex-1 flex overflow-hidden relative" onClick={handleClick}>
        <div
          ref={editorRef}
          className="flex-1 w-full h-full"
          style={{ minWidth: 0 }}
        />
        {loading && (
          <div className="absolute inset-0 flex items-center justify-center bg-background/80 backdrop-blur z-10">
            Загрузка файла…
          </div>
        )}
        {showAiHighlight && (
          <div className="absolute top-2 right-2 bg-green-500/10 border border-green-500/20 text-green-700 text-[10px] px-2 py-1 rounded z-10">
            AI-подсветка активна (демо)
          </div>
        )}
      </div>

      {/* Инфо-строка */}
      <div className="h-9 border-t flex items-center justify-between px-3 text-[10px] text-muted-foreground bg-muted/50">
        <div className="flex items-center gap-3">
          <span>{content.split('\n').length} строк</span>
          <span>•</span>
          <span>{Math.ceil(content.length / 4)} токенов</span>
          {aiTokensUsed > 0 && (
            <>
              <span>•</span>
              <span className="text-green-600">AI: ~{aiTokensUsed} ток.</span>
            </>
          )}
        </div>
        <div className="flex items-center gap-2">
          {symbols.length > 0 ? (
            <span>Символов: {symbols.length}</span>
          ) : (
            <span>Символы: —</span>
          )}
        </div>
      </div>

      {/* Навигация по символам */}
      {symbols.length > 0 && (
        <div className="h-10 border-t flex items-center px-3 text-[10px] text-muted-foreground bg-muted/30 overflow-x-auto gap-1">
          {symbols.map((s) => (
            <button
              key={s.id}
              className="px-2 py-1 hover:bg-muted rounded cursor-pointer transition-colors whitespace-nowrap flex items-center gap-1"
              onClick={() => {
                if (viewRef.current) {
                  const line = viewRef.current.state.doc.line(s.line)
                  viewRef.current.dispatch({
                    selection: { anchor: line.from },
                  })
                  viewRef.current.scrollTo(line.from)
                }
              }}
              title={`${s.kind}: ${s.signature || s.name} (L${s.line})`}
            >
              <span className="text-[9px]">{getSymbolIcon(s.kind)}</span>
              <span className="truncate max-w-[150px]">{s.name}</span>
              <span className="text-[9px]">L{s.line}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

function getSymbolIcon(kind: string) {
  switch (kind) {
    case 'function': return 'ƒ'
    case 'method': return 'm'
    case 'class': return 'C'
    case 'interface': return 'I'
    case 'type': return 'T'
    case 'constant': return 'C'
    case 'module': return 'M'
    default: return '•'
  }
}