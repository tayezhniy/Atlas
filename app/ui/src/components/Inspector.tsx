import { useState } from 'react'
import { File, ChevronRight, ChevronDown, Link, Clock, TrendingUp, ShieldAlert } from 'lucide-react'

interface InspectorProps {
  nodeId: string | null
}

export default function Inspector({ nodeId }: InspectorProps) {
  const [expandedSections, setExpandedSections] = useState<Set<string>>(new Set(['properties']))

  const toggleSection = (section: string) => {
    setExpandedSections((prev) => {
      const next = new Set(prev)
      if (next.has(section)) next.delete(section)
      else next.add(section)
      return next
    })
  }

  const isExpanded = (section: string) => expandedSections.has(section)

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
        <h3 className="text-sm font-semibold">{nodeId}</h3>
        <p className="text-[10px] text-muted-foreground">symbol • rust</p>
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
                <span className="font-mono truncate">src/main.rs</span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">Строки:</span>
                <span>1-42</span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">Сигнатура:</span>
                <span className="font-mono">fn main()</span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">Токены:</span>
                <span>150</span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">Ранг:</span>
                <span>0.85</span>
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
              <div className="flex items-center justify-between p-1.5 bg-blue-50 rounded">
                <div>
                  <span className="text-blue-700 font-semibold">Входящие:</span>
                  <span className="ml-2 text-blue-600">3 связи</span>
                </div>
                <Link className="text-blue-600 hover:underline" />
              </div>
              <div className="flex items-center justify-between p-1.5 bg-green-50 rounded">
                <div>
                  <span className="text-green-700 font-semibold">Исходящие:</span>
                  <span className="ml-2 text-green-600">2 связи</span>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Активность нейросети */}
        <div className="border rounded p-3">
          <div className="flex items-center gap-2 mb-3">
            <Clock className="w-3 h-3" />
            <span className="font-semibold text-xs">Запрашивала нейросеть</span>
          </div>
          <div className="text-xs text-muted-foreground">
            Узел не запрашивался нейросетью в текущей сессии.
          </div>
        </div>
      </div>

      {/* Действия */}
      <div className="border-t p-3 flex gap-2">
        <button className="flex-1 px-2 py-1.5 text-xs border rounded hover:bg-muted">
          Закладка
        </button>
        <button className="flex-1 px-2 py-1.5 text-xs border rounded hover:bg-muted">
          На граф
        </button>
        <button className="flex-1 px-2 py-1.5 text-xs border rounded hover:bg-muted text-red-500">
          Закрыть для НС
        </button>
      </div>
    </div>
  )
}