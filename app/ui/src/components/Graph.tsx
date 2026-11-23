import { useEffect, useState } from 'react'

interface GraphProps {
  selectedNode: string | null
}

export default function Graph({ selectedNode }: GraphProps) {
  const [zoom, setZoom] = useState(1)
  const [position, setPosition] = useState({ x: 0, y: 0 })

  useEffect(() => {
    // Здесь будет инициализация Sigma.js + graphology
    // Для MVP — визуальная заглушка
    console.log('Граф инициализирован, выбранный узел:', selectedNode)
  }, [selectedNode])

  return (
    <div className="h-full w-full bg-background border-t relative overflow-hidden" style={{ transform: `scale(${zoom}) translate(${position.x}px, ${position.y}px)` }}>
      {/* Сетка фона */}
      <div className="absolute inset-0 opacity-10" style={{ backgroundImage: "linear-gradient(to right, #e5e5e5 1px, transparent 1px), linear-gradient(to bottom, #e5e5e5 1px, transparent 1px)", backgroundSize: "40px 40px" }} />

      {/* Центральная заглушка графа */}
      <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
        <div className="text-center">
          <div className="w-24 h-24 rounded-full border-2 border-blue-500 flex items-center justify-center mx-auto mb-4 animate-pulse">
            <div className="w-12 h-12 rounded-full bg-blue-500/20 flex items-center justify-center">
              <svg className="w-6 h-6 text-blue-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 6.253v13m0-13C10.832 8.088 9.246 9.424 7 11.268 5.12 13.526 3 16.488 3 20c0 4.469 2.61 8.49 6.545 10.966 2.123 1.364 5.8 1.364 7.923 0 3.935-2.477 6.545-6.497 6.545-10.966 0-3.512-2.12-6.474-4-8.736A10.01 10.01 0 0 0 20 13c0-2.29-.842-4.46-2.243-6.097C16.97 7.878 14.526 7 12 7z" />
              </svg>
            </div>
          </div>
          <h3 className="text-muted-foreground text-sm">Граф проекта</h3>
          <p className="text-[10px] text-muted-foreground mt-1">
            Узлы и связи будут отображены здесь после индексации
          </p>
        </div>
      </div>

      {/* Инструкция управления */}
      <div className="absolute bottom-4 left-4 text-[10px] text-muted-foreground bg-background/80 backdrop-blur px-3 py-2 rounded border">
        <div className="flex items-center gap-2 mb-1">
          <span className="w-2 h-2 rounded-full bg-blue-500"></span> Каталог
        </div>
        <div className="flex items-center gap-2 mb-1">
          <span className="w-2 h-2 rounded-full bg-green-500"></span> Файл
        </div>
        <div className="flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-purple-500"></span> Символ
        </div>
      </div>
    </div>
  )
}