import { useState, useEffect, useRef } from 'react'
import { EditorView, basicSetup } from 'codemirror'
import { lineNumbers } from '@codemirror/view'
import { EditorState } from '@codemirror/state'
import { rust } from '@codemirror/lang-rust'
import { python } from '@codemirror/lang-python'
import { javascript } from '@codemirror/lang-javascript'
import { json } from '@codemirror/lang-json'
import { markdown } from '@codemirror/lang-markdown'
import { oneDark } from '@codemirror/theme-one-dark'
import { Copy, Link2 } from 'lucide-react'
import type { FileContent, SymbolNode } from '../types'

interface FileViewProps {
  projectPath: string
  /** путь файла, `путь::имя` или идентификатор узла */
  nodeId: string
  onGoToDefinition: (symbol: string, path?: string) => void
  onGraphHighlight: (nodeId: string) => void
}

type ViewMode = 'full' | 'folded' | 'signature' | 'doc'

const MODE_TITLES: Record<ViewMode, string> = {
  full: 'Полный код',
  folded: 'Свёрнуто (только каркас)',
  signature: 'Только сигнатуры',
  doc: 'Сигнатуры и документация',
}

function languageSupport(path: string) {
  switch (path.split('.').pop()) {
    case 'rs':
      return rust()
    case 'py':
      return python()
    case 'ts':
    case 'tsx':
      return javascript({ jsx: true, typescript: true })
    case 'js':
    case 'jsx':
      return javascript({ jsx: true })
    case 'json':
      return json()
    case 'md':
      return markdown()
    default:
      return []
  }
}

export default function FileView({ projectPath, nodeId, onGoToDefinition, onGraphHighlight }: FileViewProps) {
  const editorRef = useRef<HTMLDivElement>(null)
  const viewRef = useRef<EditorView | null>(null)
  const [file, setFile] = useState<FileContent | null>(null)
  const [symbols, setSymbols] = useState<SymbolNode[]>([])
  const [mode, setMode] = useState<ViewMode>('full')
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let stale = false
    setError(null)
    setFile(null)
    window.atlasAPI
      .read({ projectPath, target: nodeId, mode })
      .then(async (result) => {
        const outline = await window.atlasAPI.outline({ projectPath, path: result.path })
        if (stale) return
        setFile(result)
        setSymbols(outline.symbols)
      })
      .catch((err) => {
        if (!stale) setError(err.message)
      })
    return () => {
      stale = true
    }
  }, [projectPath, nodeId, mode])

  useEffect(() => {
    if (!editorRef.current || !file) return
    // В свёрнутых режимах строки идут не подряд: номера берём из ответа
    const numbers = file.lines.map((l) => l.line)
    const view = new EditorView({
      parent: editorRef.current,
      state: EditorState.create({
        doc: file.lines.map((l) => l.text).join('\n'),
        extensions: [
          lineNumbers({ formatNumber: (n) => String(numbers[n - 1] ?? n) }),
          basicSetup,
          EditorState.readOnly.of(true),
          EditorView.lineWrapping,
          languageSupport(file.path),
          oneDark,
          EditorView.theme({ '&': { height: '100%', fontSize: '13px' } }),
        ],
      }),
    })
    viewRef.current = view
    return () => {
      view.destroy()
      viewRef.current = null
    }
  }, [file])

  // Переход к определению: Ctrl+Click по имени
  const handleClick = (e: React.MouseEvent) => {
    const view = viewRef.current
    if (!view || !(e.ctrlKey || e.metaKey)) return
    const pos = view.posAtCoords({ x: e.clientX, y: e.clientY })
    if (pos === null) return
    const word = view.state.wordAt(pos)
    if (word) onGoToDefinition(view.state.sliceDoc(word.from, word.to))
  }

  const scrollToSymbol = (symbol: SymbolNode) => {
    const view = viewRef.current
    if (!view || !file) return
    const index = file.lines.findIndex((l) => l.line >= (symbol.line_start ?? 1))
    if (index < 0) return
    const line = view.state.doc.line(index + 1)
    view.dispatch({ selection: { anchor: line.from }, effects: EditorView.scrollIntoView(line.from, { y: 'start' }) })
  }

  const path = file?.path ?? nodeId

  return (
    <div className="h-full flex flex-col bg-background">
      <div className="h-9 border-b flex items-center justify-between px-3 text-[11px] bg-muted/50">
        <span className="truncate font-medium flex-1" title={path}>{path}</span>
        <div className="flex items-center gap-1 ml-4">
          <div className="flex border rounded bg-background" role="group" aria-label="Режим просмотра">
            {(Object.keys(MODE_TITLES) as ViewMode[]).map((m) => (
              <button
                key={m}
                onClick={() => setMode(m)}
                className={`px-2 py-1 text-[10px] border-r last:border-0 ${
                  mode === m ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-muted'
                }`}
                title={MODE_TITLES[m]}
              >
                {m.charAt(0).toUpperCase()}
              </button>
            ))}
          </div>
          <button onClick={() => navigator.clipboard.writeText(path)} className="p-1.5 rounded text-muted-foreground hover:bg-muted" title="Копировать путь">
            <Copy className="w-3 h-3" />
          </button>
          <button onClick={() => onGraphHighlight(nodeId)} className="p-1.5 rounded text-muted-foreground hover:bg-muted" title="Подсветить на графе">
            <Link2 className="w-3 h-3" />
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-hidden relative" onClick={handleClick}>
        {error && <div className="p-4 text-xs text-red-600">{error}</div>}
        {!error && !file && <div className="p-4 text-xs text-muted-foreground">Загрузка файла…</div>}
        <div ref={editorRef} className="h-full w-full" />
      </div>

      <div className="h-8 border-t flex items-center justify-between px-3 text-[10px] text-muted-foreground bg-muted/50">
        <span>{file ? `${file.total} строк в файле, показано ${file.lines.length}` : '—'}</span>
        <span>Символов: {symbols.length}</span>
      </div>

      {symbols.length > 0 && (
        <div className="h-10 border-t flex items-center px-3 text-[10px] text-muted-foreground bg-muted/30 overflow-x-auto gap-1">
          {symbols.map((s) => (
            <button
              key={s.id}
              className="px-2 py-1 hover:bg-muted rounded whitespace-nowrap"
              onClick={() => scrollToSymbol(s)}
              title={`${s.kind}: ${s.signature || s.name} (L${s.line_start})`}
            >
              {s.name} L{s.line_start}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
