import { useState, useEffect } from 'react'

interface FileViewProps {
  nodeId: string | null
}

export default function FileView({ nodeId }: FileViewProps) {
  const [content, setContent] = useState<string>('')
  const [language, setLanguage] = useState<string>('text')
  const [lineNumbers, setLineNumbers] = useState<number[]>([])

  useEffect(() => {
    // Заглушка — в будущем чтение из API
    if (nodeId) {
      setContent(`// Файл: ${nodeId}\n// Это заглушка просмотра файла\n// В будущем здесь будет подсветка синтаксиса CodeMirror 6\n\nfn main() {\n    println!("Hello, Atlas!");\n}\n`)
      setLanguage('rust')
      const lines = content.split('\n').length
      setLineNumbers(Array.from({ length: lines }, (_, i) => i + 1))
    }
  }, [nodeId, content])

  return (
    <div className="h-full flex flex-col bg-background">
      {/* Хлебные крошки */}
      <div className="h-8 border-b flex items-center gap-2 px-3 text-[10px] text-muted-foreground bg-muted/50">
        <span>src/main.rs</span>
        <span>/</span>
        <span className="text-blue-500">main</span>
        <span>/</span>
        <span className="text-blue-500">fn main</span>
      </div>

      {/* Редактор кода (только для чтения) */}
      <div className="flex-1 flex overflow-hidden">
        {/* Номера строк */}
        <div className="w-14 border-r flex-shrink-0 text-[11px] text-muted-foreground bg-muted/30 select-none">
          <div className="flex-1 flex flex-col">
            {lineNumbers.map((n) => (
              <div key={n} className="h-6 flex items-center justify-end pr-2">
                {n}
              </div>
            ))}
          </div>
        </div>

        {/* Код */}
        <div className="flex-1 overflow-auto p-4 font-mono text-sm leading-relaxed">
          <pre className="whitespace-pre"><code>{content}</code></pre>
        </div>
      </div>

      {/* Информация о символе */}
      <div className="h-12 border-t flex items-center justify-between px-3 text-[10px] text-muted-foreground bg-muted/50">
        <div className="flex items-center gap-2">
          <span>5 строк</span>
          <span>•</span>
          <span>0 токенов</span>
        </div>
        <div className="flex items-center gap-2">
          <span>Вызовы: 0</span>
          <span>•</span>
          <span>Тест: отсутствует</span>
        </div>
      </div>
    </div>
  )
}