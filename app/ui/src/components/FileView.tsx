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

export default function FileView({ nodeId }: FileViewProps) {
  const editorRef = useRef<HTMLDivElement>(null)
  const viewRef = useRef<EditorView | null>(null)
  const [language, setLanguage] = useState<string>('text')
  const [content, setContent] = useState<string>('')

  // Создаём редактор при монтировании
  useEffect(() => {
    if (!editorRef.current) return

    const state = EditorState.create({
      doc: content,
      extensions: [
        basicSetup,
        EditorView.editable.of(false),
        EditorView.lineWrapping,
        getLanguageSupport(language),
        oneDark,
        EditorView.theme({
          '&': {
            height: '100%',
            fontSize: '13px',
            lineHeight: '1.6',
          },
          '.cm-gutters': {
            backgroundColor: 'transparent',
            borderRight: '1px solid #374151',
            minWidth: '3.5rem',
            paddingRight: '0.5rem',
          },
          '.cm-lineNumbers': {
            color: '#9ca3af',
          },
        }),
      ],
    })

    const view = new EditorView({
      state,
      parent: editorRef.current,
    })
    viewRef.current = view

    return () => {
      view.destroy()
    }
  }, [])

  // Обновляем содержимое и язык при изменении nodeId
  useEffect(() => {
    if (!nodeId || !viewRef.current) return

    // Заглушка — в будущем чтение из API (MCP read tool)
    const mockContent = `// Файл: ${nodeId}
// Это пример содержимого файла
// В будущем здесь будет загрузка через MCP read tool

fn main() {
    println!("Hello, Atlas!");
}

fn helper() -> i32 {
    return 42;
}

struct Config {
    name: String,
    value: i32,
}

impl Config {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            value: 0,
        }
    }
}
`
    const mockLang = 'rust'

    setContent(mockContent)
    setLanguage(mockLang)

    viewRef.current.dispatch({
      changes: { from: 0, to: viewRef.current.state.doc.length, insert: mockContent },
    })
  }, [nodeId])

  return (
    <div className="h-full flex flex-col bg-background">
      {/* Хлебные крошки / заголовок файла */}
      <div className="h-8 border-b flex items-center gap-2 px-3 text-[10px] text-muted-foreground bg-muted/50">
        <span>{nodeId ? nodeId.split('/').pop() || nodeId : 'Файл не выбран'}</span>
        {nodeId && <span className="text-muted-foreground">•</span>}
        {nodeId && <span className="text-blue-500 text-capitalize">{language}</span>}
      </div>

      {/* Редактор CodeMirror */}
      <div className="flex-1 flex overflow-hidden relative">
        <div
          ref={editorRef}
          className="flex-1 w-full h-full"
          style={{ minWidth: 0 }}
        />
      </div>

      {/* Информация о символе */}
      <div className="h-12 border-t flex items-center justify-between px-3 text-[10px] text-muted-foreground bg-muted/50">
        <div className="flex items-center gap-2">
          <span>{content.split('\n').length} строк</span>
          <span>•</span>
          <span>{Math.ceil(content.length / 4)} токенов</span>
        </div>
        <div className="flex items-center gap-2">
          <span>Вызовы: —</span>
          <span>•</span>
          <span>Тест: —</span>
        </div>
      </div>
    </div>
  )
}