import { useState, useEffect, useRef, useCallback } from 'react'
import { Search, FileText, Zap, Type, File, Folder, FunctionSquare, Component, Box, Copy } from 'lucide-react'
import type { SearchResult, GraphNode } from '../types'

interface CommandPaletteProps {
  projectPath: string
  isOpen: boolean
  initialMode: 'files' | 'symbols' | 'commands'
  onClose: () => void
  onSelectFile: (path: string) => void
  onSelectSymbol: (symbol: SearchResult) => void
  onRunCommand: (command: string) => void
  recentFiles: string[]
}

interface CommandItem {
  id: string
  type: 'file' | 'symbol' | 'command' | 'recent'
  label: string
  description?: string
  icon: React.ReactNode
  action: () => void
  shortcut?: string
  score?: number
}

export default function CommandPalette({
  projectPath,
  isOpen,
  initialMode,
  onClose,
  onSelectFile,
  onSelectSymbol,
  onRunCommand,
  recentFiles,
}: CommandPaletteProps) {
  const [query, setQuery] = useState('')
  const [items, setItems] = useState<CommandItem[]>([])
  const [selectedIndex, setSelectedIndex] = useState(0)
  const [mode, setMode] = useState<'files' | 'symbols' | 'commands'>('files')
  const inputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)

  // Режим по модификаторам: Ctrl+P = файлы, Ctrl+Shift+F = символы, Ctrl+K = команды
  useEffect(() => {
    if (!isOpen) return
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'p') {
        e.preventDefault()
        setMode('files')
        setQuery('')
      }
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'f') {
        e.preventDefault()
        setMode('symbols')
        setQuery('')
      }
      if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault()
        setMode('commands')
        setQuery('')
      }
      if (e.key === 'Escape') {
        onClose()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [isOpen, onClose])

  // Фокус на инпуте при открытии
  useEffect(() => {
    if (isOpen) {
      setTimeout(() => inputRef.current?.focus(), 0)
      setSelectedIndex(0)
      setMode(initialMode)
      setQuery('')
    }
  }, [isOpen, initialMode])

  // Поиск при изменении запроса
  useEffect(() => {
    if (!isOpen || !projectPath || !window.atlasAPI) return

    // список и выбор от прошлого запроса не должны срабатывать по Enter
    setSelectedIndex(0)
    if (mode === 'symbols' && query.length < 2) setItems([])

    const search = async () => {
      if (mode === 'files') {
        // Поиск файлов через overview + фильтрация
        try {
          const overview = await window.atlasAPI.overview(projectPath)
          const allFiles = overview.top.map(n => n.path)
          const filtered = allFiles
            .filter(p => p.toLowerCase().includes(query.toLowerCase()))
            .slice(0, 20)
            .map((path, idx) => ({
              id: `file-${idx}`,
              type: 'file' as const,
              label: path.split('/').pop() || path,
              description: path,
              icon: path.endsWith('/') ? <Folder className="w-4 h-4" /> : <FileText className="w-4 h-4" />,
              action: () => { onSelectFile(path); onClose(); },
              score: 100 - idx,
            }))
          // Добавляем недавние файлы сверху
          const recent = recentFiles
            .filter(f => !filtered.some(item => item.description === f))
            .slice(0, 5)
            .map((path, idx) => ({
              id: `recent-${idx}`,
              type: 'recent' as const,
              label: path.split('/').pop() || path,
              description: path,
              icon: <File className="w-4 h-4" />,
              action: () => { onSelectFile(path); onClose(); },
              score: 200 - idx,
            }))
          setItems([...recent, ...filtered])
        } catch {
          setItems([])
        }
      } else if (mode === 'symbols' && query.length >= 2) {
        // Поиск символов через MCP search
        try {
          const result = await window.atlasAPI.search({ projectPath, query, limit: 30 })
          const mapped = result.items.map((item, idx) => ({
            id: `symbol-${item.id}`,
            type: 'symbol' as const,
            label: item.name,
            description: `${item.path}:${item.line_start ?? 0} (${item.kind})`,
            icon: getSymbolIcon(item.kind),
            action: () => { onSelectSymbol(item); onClose(); },
            score: item.score,
          }))
          setItems(mapped)
        } catch {
          setItems([])
        }
      } else if (mode === 'commands') {
        // Команды
        const commands: CommandItem[] = [
          {
            id: 'cmd-index',
            type: 'command',
            label: 'Проиндексировать проект',
            description: 'Полная переиндексация (atlas index)',
            icon: <Zap className="w-4 h-4" />,
            action: () => { onRunCommand('index'); onClose(); },
          },
          {
            id: 'cmd-watch',
            type: 'command',
            label: 'Запустить watch-режим',
            description: 'Автоиндексация при изменениях (atlas watch)',
            icon: <Zap className="w-4 h-4" />,
            action: () => { onRunCommand('watch'); onClose(); },
          },
          {
            id: 'cmd-doctor',
            type: 'command',
            label: 'Проверить состояние (doctor)',
            description: 'Диагностика индекса и БД',
            icon: <Search className="w-4 h-4" />,
            action: () => { onRunCommand('doctor'); onClose(); },
          },
          {
            id: 'cmd-export-json',
            type: 'command',
            label: 'Экспорт отчёта JSON',
            description: 'Сохранить граф и статистику в JSON',
            icon: <Copy className="w-4 h-4" />,
            action: () => { onRunCommand('export-json'); onClose(); },
          },
          {
            id: 'cmd-export-md',
            type: 'command',
            label: 'Экспорт отчёта Markdown',
            description: 'Сохранить отчёт в Markdown',
            icon: <Copy className="w-4 h-4" />,
            action: () => { onRunCommand('export-md'); onClose(); },
          },
          {
            id: 'cmd-settings',
            type: 'command',
            label: 'Открыть настройки',
            description: 'Исключения, бюджеты, тема',
            icon: <Search className="w-4 h-4" />,
            action: () => { onRunCommand('settings'); onClose(); },
            shortcut: 'Ctrl+,',
          },
        ]
        const filtered = commands
          .filter(c => c.label.toLowerCase().includes(query.toLowerCase()) || c.description?.toLowerCase().includes(query.toLowerCase()))
        setItems(filtered)
      }
    }

    const debounce = setTimeout(search, 100)
    return () => clearTimeout(debounce)
  }, [query, mode, projectPath, isOpen, onSelectFile, onSelectSymbol, onRunCommand, onClose, recentFiles])

  // Навигация клавишами
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setSelectedIndex(i => Math.min(i + 1, items.length - 1))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setSelectedIndex(i => Math.max(i - 1, 0))
    } else if (e.key === 'Enter' && items[selectedIndex]) {
      e.preventDefault()
      items[selectedIndex].action()
    } else if (e.key === 'Escape') {
      onClose()
    }
  }, [items, selectedIndex, onClose])

  if (!isOpen) return null

  const modeLabels = { files: 'Файлы (Ctrl+P)', symbols: 'Символы (Ctrl+Shift+F)', commands: 'Команды (Ctrl+K)' }

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-16 pointer-events-none" onClick={onClose}>
      <div className="pointer-events-auto w-full max-w-3xl bg-background border shadow-xl rounded-lg overflow-hidden animate-slide-down">
        {/* Заголовок с табами режимов */}
        <div className="border-b bg-muted/50 px-4 py-2 flex items-center gap-4">
          <div className="flex items-center gap-2 flex-1">
            <Search className="w-4 h-4 text-muted-foreground" />
            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={e => setQuery(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={`Поиск ${modeLabels[mode]}…`}
              className="flex-1 bg-transparent border-none outline-none text-sm text-foreground placeholder:text-muted-foreground"
              spellCheck={false}
            />
            <kbd className="px-2 py-0.5 text-[10px] bg-muted rounded text-muted-foreground font-mono">Esc</kbd>
          </div>
          <div className="flex gap-1" role="tablist">
            {(['files', 'symbols', 'commands'] as const).map(m => (
              <button
                key={m}
                role="tab"
                aria-selected={mode === m}
                onClick={() => { setMode(m); setQuery(''); }}
                className={`px-3 py-1 text-[11px] rounded transition-colors ${
                  mode === m ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-muted'
                }`}
              >
                {m === 'files' && <FileText className="w-3 h-3 mr-1" />}
                {m === 'symbols' && <FunctionSquare className="w-3 h-3 mr-1" />}
                {m === 'commands' && <Zap className="w-3 h-3 mr-1" />}
                {modeLabels[m]}
              </button>
            ))}
          </div>
        </div>

        {/* Список результатов */}
        <div
          ref={listRef}
          className="max-h-[50vh] overflow-y-auto"
          role="listbox"
        >
          {items.length === 0 && (
            <div className="p-8 text-center text-muted-foreground text-sm">
              {query ? 'Ничего не найдено' : 'Введите запрос для поиска'}
            </div>
          )}
          {items.map((item, idx) => (
            <div
              key={item.id}
              role="option"
              aria-selected={idx === selectedIndex}
              className={`flex items-center gap-3 px-4 py-2.5 hover:bg-accent cursor-pointer transition-colors ${
                idx === selectedIndex ? 'bg-accent' : ''
              } border-b last:border-0`}
              onClick={item.action}
              onMouseEnter={() => setSelectedIndex(idx)}
            >
              <span className="text-muted-foreground">{item.icon}</span>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 truncate">
                  <span className="font-medium text-sm truncate">{item.label}</span>
                  {item.shortcut && (
                    <kbd className="px-1.5 py-0.5 text-[10px] bg-muted rounded text-muted-foreground font-mono ml-auto shrink-0">
                      {item.shortcut}
                    </kbd>
                  )}
                </div>
                {item.description && (
                  <div className="text-[11px] text-muted-foreground truncate">{item.description}</div>
                )}
              </div>
              {item.type === 'recent' && (
                <span className="text-[10px] text-green-600 px-1.5 py-0.5 rounded bg-green-50">недавний</span>
              )}
            </div>
          ))}
        </div>

        {/* Подсказки */}
        <div className="px-4 py-2 border-t bg-muted/30 text-[10px] text-muted-foreground flex flex-wrap gap-4">
          <span>↑↓ навигация</span>
          <span>Enter — открыть</span>
          <span>Esc — закрыть</span>
          <span>Ctrl+P файлы</span>
          <span>Ctrl+Shift+F символы</span>
          <span>Ctrl+K команды</span>
        </div>
      </div>
    </div>
  )
}

function getSymbolIcon(kind: string): React.ReactNode {
  switch (kind) {
    case 'function': return <FunctionSquare className="w-4 h-4 text-yellow-500" />
    case 'method': return <FunctionSquare className="w-4 h-4 text-blue-500" />
    case 'class': return <Box className="w-4 h-4 text-purple-500" />
    case 'interface': return <Component className="w-4 h-4 text-cyan-500" />
    case 'type': return <Type className="w-4 h-4 text-orange-500" />
    case 'constant': return <Zap className="w-4 h-4 text-red-500" />
    case 'module': return <Folder className="w-4 h-4 text-gray-500" />
    default: return <FileText className="w-4 h-4" />
  }
}