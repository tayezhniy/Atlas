import { useState, useEffect, useRef } from 'react'
import { EditorView, basicSetup } from 'codemirror'
import { EditorState } from '@codemirror/state'
import { rust } from '@codemirror/lang-rust'
import { python } from '@codemirror/lang-python'
import { javascript } from '@codemirror/lang-javascript'
import { json } from '@codemirror/lang-json'
import { markdown } from '@codemirror/lang-markdown'
import { toml } from '@codemirror/lang-toml'
import { oneDark } from '@codemirror/theme-one-dark'

interface FileViewProps {
  projectPath: string
  nodeId: string | null
}

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

export default function FileView({ projectPath, nodeId }: FileViewProps) {
  const editorRef = useRef<HTMLDivElement>(null)
  const viewRef = useRef<EditorView | null>(null)
  const [language, setLanguage] = useState<string>('text')
  const [content, setContent] = useState<string>('')
  const [loading, setLoading] = useState(false)
  const [symbols, setSymbols] = useState<Array<{ name: string; line: number }>>([])

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
        EditorView.theme({
          '&': { height: '100%', fontSize: '13px', lineHeight: '1.6' },
          '.cm-gutters': {
            backgroundColor: 'transparent',
            borderRight: '1px solid #374151',
            minWidth: '3.5rem',
            paddingRight: '0.5rem',
          },
          '.cm-lineNumbers': { color: '#9ca3af' },
        }),
      ],
    })

    const view = new EditorView({ state, parent: editorRef.current })
    viewRef.current = view

    return () => {
      view.destroy()
    }
  }, [language, content])

  // Загружаем содержимое файла при изменении nodeId
  useEffect(() => {
    if (!nodeId || !window.atlasAPI || !viewRef.current) return

    setLoading(true)
    window.atlasAPI
      .read({ projectPath, path: nodeId })
      .then((result) => {
        const text = result.lines.map((l) => l.text).join('\n')
        setContent(text)
        // Определяем язык по расширению
        const ext = nodeId.split('.').pop() ?? 'text'
        setLanguage(ext)
        viewRef.current?.dispatch({
          changes: { from: 0, to: viewRef.current.state.doc.length, insert: text },
        })
        setLoading(false)
      })
      .catch(() => {
        setLoading(false)
      })

    // Загружаем контур файла (символы)
    window.atlasAPI
      .outline({ projectPath, path: nodeId })
      .then((result) => {
        setSymbols(
          result.symbols.map((s) => ({ name: s.name, line: s.line_start ?? 0 }))
        )
      })
      .catch(() => {
        // outline может быть недоступен для неиндексированных файлов
      })
  }, [nodeId, projectPath])

  return (
    <div className="h-full flex flex-col bg-background">
      {/* Хлебные крошки / заголовок файла */}
      <div className="h-8 border-b flex items-center gap-2 px-3 text-[10px] text-muted-foreground bg-muted/50">
        <span>{nodeId ? nodeId.split('/').pop() || nodeId : 'Файл не выбран'}</span>
        {nodeId && <span className="text-muted-foreground">•</span>}
        {nodeId && <span className="text-blue-500 text-capitalize">{language}</span>}
        {loading && <span className="text-[10px] text-muted-foreground">• загрузка…</span>}
      </div>

      {/* Редактор CodeMirror */}
      <div className="flex-1 flex overflow-hidden relative">
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
      </div>

      {/* Информация о символе */}
      <div className="h-12 border-t flex items-center justify-between px-3 text-[10px] text-muted-foreground bg-muted/50">
        <div className="flex items-center gap-2">
          <span>{content.split('\n').length} строк</span>
          <span>•</span>
          <span>{Math.ceil(content.length / 4)} токенов</span>
        </div>
        <div className="flex items-center gap-2">
          {symbols.length > 0 ? (
            <span>Символов: {symbols.length}</span>
          ) : (
            <span>Символы: —</span>
          )}
        </div>
      </div>

      {symbols.length > 0 && (
        <div className="h-10 border-t flex items-center px-3 text-[10px] text-muted-foreground bg-muted/30 overflow-x-auto">
          {symbols.map((s) => (
            <span key={`${s.name}-${s.line}`} className="px-2 hover:bg-muted rounded cursor-pointer">
              {s.name} (L{s.line})
            </span>
          ))}
        </div>
      )}
    </div>
  )
}
