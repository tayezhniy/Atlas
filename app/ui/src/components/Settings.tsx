import { useState, useEffect, useCallback } from 'react'
import {
  Shield, Save, RotateCcw,
  Plus, X, EyeOff, Zap, Database
} from 'lucide-react'

interface SettingsProps {
  projectPath: string
  isOpen: boolean
  onClose: () => void
}

interface SettingsData {
  exclusions: string[]
  budgets: Record<string, number>
  theme: 'dark' | 'light' | 'system'
  autoIndex: boolean
  watchEnabled: boolean
  maxFileSize: number
  closedPaths: string[]
}

const DEFAULT_EXCLUSIONS = [
  '**/node_modules/**',
  '**/.git/**',
  '**/.atlas/**',
  '**/target/**',
  '**/dist/**',
  '**/build/**',
  '**/__pycache__/**',
  '**/*.pyc',
  '**/vendor/**',
  '**/.next/**',
  '**/.vercel/**',
  '**/coverage/**',
  '**/.turbo/**',
]

const DEFAULT_BUDGETS = {
  overview: 800,
  search: 600,
  outline: 1500,
  read: 2000,
  neighbors: 1000,
  context_for: 2500,
  status: 300,
}

const DEFAULT_CLOSED_PATHS = [
  '.env*',
  'id_rsa*',
  'id_ed25519*',
  '*.pem',
  '*.key',
  '*.p12',
  '*.pfx',
  '*.keystore',
  '**/credentials/**',
  '**/secrets.*',
]

export default function Settings({ projectPath, isOpen, onClose }: SettingsProps) {
  const [settings, setSettings] = useState<SettingsData>({
    exclusions: [...DEFAULT_EXCLUSIONS],
    budgets: { ...DEFAULT_BUDGETS },
    theme: 'dark',
    autoIndex: true,
    watchEnabled: false,
    maxFileSize: 1024 * 1024, // 1MB
    closedPaths: [...DEFAULT_CLOSED_PATHS],
  })
  const [activeTab, setActiveTab] = useState<'exclusions' | 'budgets' | 'general' | 'security'>('exclusions')
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)
  const [newExclusion, setNewExclusion] = useState('')
  const [newClosedPath, setNewClosedPath] = useState('')

  // Загрузка настроек при открытии
  useEffect(() => {
    if (isOpen && projectPath && window.atlasAPI) {
      loadSettings()
    }
  }, [isOpen, projectPath])

  const loadSettings = async () => {
    setLoading(true)
    try {
      const result = await window.atlasAPI!.settings.all(projectPath)
      if (result.settings) {
        setSettings(prev => ({
          ...prev,
          exclusions: result.settings.exclusions || prev.exclusions,
          budgets: { ...prev.budgets, ...result.settings.budgets },
          theme: result.settings.theme || prev.theme,
          autoIndex: result.settings.autoIndex ?? prev.autoIndex,
          watchEnabled: result.settings.watchEnabled ?? prev.watchEnabled,
          maxFileSize: result.settings.maxFileSize ?? prev.maxFileSize,
          closedPaths: result.settings.closedPaths || prev.closedPaths,
        }))
      }
    } catch {
      // Используем дефолты
    } finally {
      setLoading(false)
    }
  }

  const saveSettings = async () => {
    if (!window.atlasAPI) return
    setSaving(true)
    try {
      // Сохраняем каждую настройку отдельно
      for (const [key, value] of Object.entries(settings)) {
        await window.atlasAPI.settings.set(projectPath, key, JSON.stringify(value))
      }
    } catch (err) {
      console.error('Failed to save settings:', err)
    } finally {
      setSaving(false)
    }
  }

  const resetToDefaults = () => {
    setSettings({
      exclusions: [...DEFAULT_EXCLUSIONS],
      budgets: { ...DEFAULT_BUDGETS },
      theme: 'dark',
      autoIndex: true,
      watchEnabled: false,
      maxFileSize: 1024 * 1024,
      closedPaths: [...DEFAULT_CLOSED_PATHS],
    })
  }

  const addExclusion = () => {
    if (newExclusion.trim() && !settings.exclusions.includes(newExclusion.trim())) {
      setSettings(prev => ({
        ...prev,
        exclusions: [...prev.exclusions, newExclusion.trim()],
      }))
      setNewExclusion('')
    }
  }

  const removeExclusion = (index: number) => {
    setSettings(prev => ({
      ...prev,
      exclusions: prev.exclusions.filter((_, i) => i !== index),
    }))
  }

  const addClosedPath = () => {
    if (newClosedPath.trim() && !settings.closedPaths.includes(newClosedPath.trim())) {
      setSettings(prev => ({
        ...prev,
        closedPaths: [...prev.closedPaths, newClosedPath.trim()],
      }))
      setNewClosedPath('')
    }
  }

  const removeClosedPath = (index: number) => {
    setSettings(prev => ({
      ...prev,
      closedPaths: prev.closedPaths.filter((_, i) => i !== index),
    }))
  }

  const updateBudget = (tool: string, value: number) => {
    setSettings(prev => ({
      ...prev,
      budgets: { ...prev.budgets, [tool]: Math.max(50, value) },
    }))
  }

  const tabs = [
    { id: 'exclusions', label: 'Исключения', icon: <EyeOff className="w-4 h-4" /> },
    { id: 'budgets', label: 'Бюджеты токенов', icon: <Zap className="w-4 h-4" /> },
    { id: 'security', label: 'Безопасность', icon: <Shield className="w-4 h-4" /> },
    { id: 'general', label: 'Общие', icon: <Database className="w-4 h-4" /> },
  ]

  if (!isOpen) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center pointer-events-none" onClick={onClose}>
      <div className="pointer-events-auto w-full max-w-3xl max-h-[85vh] bg-background border shadow-xl rounded-lg overflow-hidden flex flex-col animate-slide-down">
        {/* Заголовок */}
        <div className="flex items-center justify-between p-4 border-b bg-muted/50">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded bg-blue-500 flex items-center justify-center">
              <Database className="w-4 h-4 text-white" />
            </div>
            <h2 className="text-lg font-semibold">Настройки проекта</h2>
            <span className="text-xs text-muted-foreground px-2 py-0.5 bg-muted rounded">{projectPath}</span>
          </div>
          <div className="flex items-center gap-2">
            <button onClick={resetToDefaults} disabled={saving} className="px-3 py-1.5 text-xs border rounded hover:bg-muted disabled:opacity-50">
              <RotateCcw className="w-3 h-3 mr-1" /> Сбросить
            </button>
            <button onClick={saveSettings} disabled={saving} className="px-3 py-1.5 text-xs bg-primary text-primary-foreground rounded hover:bg-primary/90 disabled:opacity-50 flex items-center gap-1">
              <Save className="w-3 h-3" /> {saving ? 'Сохранение…' : 'Сохранить'}
            </button>
            <button onClick={onClose} className="p-2 rounded hover:bg-muted text-muted-foreground">
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Табы */}
        <div className="border-b bg-muted/30 flex overflow-x-auto" role="tablist">
          {tabs.map(tab => (
            <button
              key={tab.id}
              role="tab"
              aria-selected={activeTab === tab.id}
              onClick={() => setActiveTab(tab.id as any)}
              className={`flex items-center gap-2 px-4 py-2.5 text-sm border-b-2 transition-colors whitespace-nowrap ${
                activeTab === tab.id
                  ? 'border-primary text-primary font-medium'
                  : 'text-muted-foreground hover:text-foreground border-transparent'
              }`}
            >
              {tab.icon}
              <span>{tab.label}</span>
            </button>
          ))}
        </div>

        {/* Контент табов */}
        <div className="flex-1 overflow-y-auto p-4 space-y-6" role="tabpanel">
          {/* Исключения */}
          {activeTab === 'exclusions' && (
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <h3 className="font-medium">Глобсы исключений</h3>
                <span className="text-xs text-muted-foreground">{settings.exclusions.length} правил</span>
              </div>
              <p className="text-xs text-muted-foreground">
                Файлы и каталоги, соответствующие этим паттернам, не будут индексироваться.
                Используйте <code className="px-1 bg-muted rounded">**</code> для рекурсивного совпадения.
              </p>
              <div className="flex gap-2 mb-3">
                <input
                  type="text"
                  value={newExclusion}
                  onChange={e => setNewExclusion(e.target.value)}
                  onKeyDown={e => e.key === 'Enter' && addExclusion()}
                  placeholder="Добавить glob (например, **/*.min.js)"
                  className="flex-1 px-3 py-2 text-sm border rounded bg-input"
                />
                <button onClick={addExclusion} className="px-4 py-2 bg-primary text-primary-foreground rounded hover:bg-primary/90 text-sm">
                  <Plus className="w-4 h-4 mr-1" /> Добавить
                </button>
              </div>
              <div className="max-h-60 overflow-y-auto border rounded bg-muted/30">
                {settings.exclusions.length === 0 ? (
                  <div className="p-4 text-center text-xs text-muted-foreground">Нет правил исключения</div>
                ) : (
                  <ul className="divide-y">
                    {settings.exclusions.map((pattern, idx) => (
                      <li key={idx} className="flex items-center justify-between px-3 py-2 hover:bg-muted/50">
                        <code className="text-sm font-mono text-foreground truncate flex-1">{pattern}</code>
                        <button
                          onClick={() => removeExclusion(idx)}
                          className="p-1.5 text-muted-foreground hover:text-red-500 hover:bg-red-500/10 rounded transition-colors ml-2"
                          title="Удалить"
                        >
                          <X className="w-3 h-3" />
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
              <details className="text-xs text-muted-foreground">
                <summary className="cursor-pointer mb-2">Предустановленные исключения (нажмите, чтобы показать)</summary>
                <ul className="space-y-1 ml-4">
                  {DEFAULT_EXCLUSIONS.map(p => (
                    <li key={p}><code className="text-[11px]">{p}</code></li>
                  ))}
                </ul>
              </details>
            </div>
          )}

          {/* Бюджеты токенов */}
          {activeTab === 'budgets' && (
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <h3 className="font-medium">Бюджеты токенов на инструмент</h3>
                <span className="text-xs text-muted-foreground">Максимальный размер ответа MCP</span>
              </div>
              <p className="text-xs text-muted-foreground">
                Ограничьте размер ответов, чтобы не перегружать контекст нейросети.
                Значения в токенах (1 токен ≈ 4 символа).
              </p>
              <div className="space-y-3">
                {Object.entries(DEFAULT_BUDGETS).map(([tool, defaultValue]) => (
                  <div key={tool} className="flex items-center gap-4 p-3 bg-muted/30 rounded">
                    <div className="w-36 font-mono text-sm text-muted-foreground">{tool}</div>
                    <input
                      type="number"
                      value={settings.budgets[tool] || defaultValue}
                      onChange={e => updateBudget(tool, parseInt(e.target.value) || defaultValue)}
                      min={50}
                      max={10000}
                      className="w-24 px-2 py-1.5 text-sm border rounded bg-input text-right"
                    />
                    <input
                      type="range"
                      value={settings.budgets[tool] || defaultValue}
                      onChange={e => updateBudget(tool, parseInt(e.target.value))}
                      min={50}
                      max={10000}
                      step={50}
                      className="flex-1"
                    />
                    <span className="text-xs text-muted-foreground w-16 text-right">
                      ~{Math.round((settings.budgets[tool] || defaultValue) / 4)}K симв.
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Безопасность */}
          {activeTab === 'security' && (
            <div className="space-y-6">
              <div className="space-y-4">
                <h3 className="font-medium">Закрытые пути (секреты)</h3>
                <p className="text-xs text-muted-foreground">
                  Эти файлы никогда не будут отданы через MCP, даже при прямом запросе.
                  Значения, похожие на ключи/токены, автоматически маскируются.
                </p>
                <div className="flex gap-2 mb-3">
                  <input
                    type="text"
                    value={newClosedPath}
                    onChange={e => setNewClosedPath(e.target.value)}
                    onKeyDown={e => e.key === 'Enter' && addClosedPath()}
                    placeholder="Добавить паттерн (например, *.secret)"
                    className="flex-1 px-3 py-2 text-sm border rounded bg-input"
                  />
                  <button onClick={addClosedPath} className="px-4 py-2 bg-primary text-primary-foreground rounded hover:bg-primary/90 text-sm">
                    <Plus className="w-4 h-4 mr-1" /> Добавить
                  </button>
                </div>
                <div className="max-h-48 overflow-y-auto border rounded bg-muted/30">
                  {settings.closedPaths.map((pattern, idx) => (
                    <div key={idx} className="flex items-center justify-between px-3 py-2 hover:bg-muted/50 border-b last:border-0">
                      <code className="text-sm font-mono text-red-500 truncate flex-1">{pattern}</code>
                      <button
                        onClick={() => removeClosedPath(idx)}
                        className="p-1.5 text-muted-foreground hover:text-red-500 hover:bg-red-500/10 rounded transition-colors ml-2"
                        title="Удалить"
                      >
                        <X className="w-3 h-3" />
                      </button>
                    </div>
                  ))}
                </div>
              </div>

              <div className="border-t pt-4">
                <h3 className="font-medium mb-3">Маскировка секретов</h3>
                <p className="text-xs text-muted-foreground mb-3">
                  Автоматически маскируются значения после ключевых слов:
                </p>
                <div className="grid grid-cols-2 gap-2 text-xs">
                  {['api_key', 'apikey', 'secret', 'password', 'access_token', 'auth_token', 'private_key', 'authorization', 'bearer'].map(k => (
                    <code key={k} className="px-2 py-1 bg-muted rounded">{k}</code>
                  ))}
                </div>
                <p className="text-xs text-muted-foreground mt-3">
                  Также маскируются известные префиксы токенов: <code className="px-1 bg-muted rounded">sk-</code>, <code className="px-1 bg-muted rounded">AKIA</code>, <code className="px-1 bg-muted rounded">ghp_</code>, <code className="px-1 bg-muted rounded">xoxb-</code>.
                </p>
              </div>
            </div>
          )}

          {/* Общие */}
          {activeTab === 'general' && (
            <div className="space-y-6">
              <div className="space-y-4">
                <h3 className="font-medium">Поведение</h3>
                <label className="flex items-center justify-between p-3 bg-muted/30 rounded cursor-pointer">
                  <div>
                    <div className="font-medium">Автоиндексация при открытии</div>
                    <div className="text-xs text-muted-foreground">Автоматически индексировать проект при первом открытии, если индекса нет</div>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.autoIndex}
                    onChange={e => setSettings(prev => ({ ...prev, autoIndex: e.target.checked }))}
                    className="w-4 h-4 accent-blue-500"
                  />
                </label>
                <label className="flex items-center justify-between p-3 bg-muted/30 rounded cursor-pointer">
                  <div>
                    <div className="font-medium">Watch-режим (автообновление)</div>
                    <div className="text-xs text-muted-foreground">Следить за изменениями файлов и переиндексировать инкрементально</div>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.watchEnabled}
                    onChange={e => setSettings(prev => ({ ...prev, watchEnabled: e.target.checked }))}
                    className="w-4 h-4 accent-blue-500"
                  />
                </label>
              </div>

              <div className="space-y-4">
                <h3 className="font-medium">Ограничения</h3>
                <div className="p-3 bg-muted/30 rounded">
                  <label className="block text-sm font-medium mb-2">Максимальный размер файла для индексации</label>
                  <div className="flex items-center gap-4">
                    <input
                      type="number"
                      value={settings.maxFileSize / 1024}
                      onChange={e => setSettings(prev => ({ ...prev, maxFileSize: Math.max(1, parseInt(e.target.value) || 1) * 1024 }))}
                      min={1}
                      max={10240}
                      className="w-24 px-2 py-1.5 text-sm border rounded bg-input"
                    />
                    <span className="text-sm text-muted-foreground">KB (текущее: {(settings.maxFileSize / 1024).toFixed(0)} KB)</span>
                    <input
                      type="range"
                      value={settings.maxFileSize / 1024}
                      onChange={e => setSettings(prev => ({ ...prev, maxFileSize: parseInt(e.target.value) * 1024 }))}
                      min={1}
                      max={10240}
                      className="flex-1"
                    />
                  </div>
                </div>
              </div>

              <div className="space-y-4">
                <h3 className="font-medium">Тема</h3>
                <div className="flex gap-3">
                  {(['dark', 'light', 'system'] as const).map(t => (
                    <label key={t} className="flex-1 p-3 border rounded cursor-pointer transition-colors flex items-center gap-2">
                      <input
                        type="radio"
                        name="theme"
                        checked={settings.theme === t}
                        onChange={() => setSettings(prev => ({ ...prev, theme: t }))}
                        className="accent-blue-500"
                      />
                      <span className="capitalize">{t === 'system' ? 'Система' : t}</span>
                    </label>
                  ))}
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Статусная строка */}
        <div className="border-t p-3 bg-muted/30 flex items-center justify-between text-xs text-muted-foreground">
          <span>Настройки хранятся в .atlas/index.db (таблица settings)</span>
          <span>Изменения вступят в силу после сохранения</span>
        </div>
      </div>
    </div>
  )
}