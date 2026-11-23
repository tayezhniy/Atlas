import { useState } from 'react'
import { BarChart2, Clock, Coin, AlertTriangle } from 'lucide-react'

export default function TokenPanel() {
  const [period, setPeriod] = useState<'session' | 'day' | 'week'>('session')

  return (
    <div className="border-t p-3 flex flex-col h-96 overflow-hidden">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-xs font-semibold">Расход токенов</h3>
        <div className="flex gap-1">
          {['session', 'day', 'week'].map((p) => (
            <button
              key={p}
              onClick={() => setPeriod(p as any)}
              className={`px-2 py-0.5 text-[10px] rounded transition-colors ${
                period === p ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-muted'
              }`}
            >
              {p === 'session' ? 'Сессия' : p === 'day' ? 'День' : 'Неделя'}
            </button>
          ))}
        </div>
      </div>

      {/* Сводка */}
      <div className="grid grid-cols-2 gap-2 mb-3">
        <div className="p-2 bg-muted/50 rounded border">
          <div className="text-[10px] text-muted-foreground">Отдано нейросети</div>
          <div className="text-xl font-mono font-bold">0</div>
        </div>
        <div className="p-2 bg-muted/50 rounded border">
          <div className="text-[10px] text-muted-foreground">Экономия</div>
          <div className="text-xl font-mono font-bold text-green-600">0%</div>
        </div>
      </div>

      {/* График (заглушка) */}
      <div className="flex-1 mb-3 border rounded p-2 bg-muted/30">
        <BarChart2 className="w-5 h-5 text-muted-foreground mx-auto mb-1" />
        <div className="text-[10px] text-muted-foreground text-center">
          График расхода по времени будет здесь
        </div>
      </div>

      {/* Топ вызовов */}
      <div className="border rounded p-2 bg-muted/30">
        <div className="flex items-center justify-between mb-2">
          <span className="text-[10px] font-semibold">Самые дорогие ответы</span>
          <span className="text-[10px] text-muted-foreground">CSV</span>
        </div>
        <div className="text-[10px] text-muted-foreground text-center py-2">
          Нет данных — выполните запросы через MCP
        </div>
      </div>
    </div>
  )
}