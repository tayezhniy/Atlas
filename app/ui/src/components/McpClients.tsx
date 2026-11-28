import { useState } from 'react'
import { Check, ChevronDown, Copy, ExternalLink, Plug, Terminal } from 'lucide-react'

type ClientId = 'goose' | 'opencode' | 'claude-desktop' | 'cursor' | 'continue' | 'custom'

interface ClientConfig {
  id: ClientId
  name: string
  icon: string
  description: string
  configFormat: 'json' | 'yaml' | 'toml'
  filePath: string
  buildConfig: (binary: string, project: string) => string
  docsUrl?: string
}

const CLIENTS: ClientConfig[] = [
  {
    id: 'goose',
    name: 'Goose',
    icon: '🪿',
    description: 'Открытый AI-агент от Block. Поддерживает MCP через конфиг в ~/.config/goose/config.yaml.',
    configFormat: 'yaml',
    filePath: '~/.config/goose/config.yaml',
    buildConfig: (bin, proj) =>
      `mcp_servers:\n  atlas:\n    command: ${bin}\n    args:\n      - mcp\n      - --project\n      - ${proj}`,
    docsUrl: 'https://block.github.io/goose/docs/getting-started/using-mcp',
  },
  {
    id: 'opencode',
    name: 'OpenCode',
    icon: '🌐',
    description: 'Терминальный AI-кодер с поддержкой MCP. Конфиг в opencode.json или ~/.config/opencode/config.json.',
    configFormat: 'json',
    filePath: 'opencode.json (в корне проекта) или ~/.config/opencode/config.json',
    buildConfig: (bin, proj) =>
      JSON.stringify(
        { mcpServers: { atlas: { command: bin, args: ['mcp', '--project', proj] } } },
        null,
        2,
      ),
    docsUrl: 'https://opencode.ai/docs/mcp',
  },
  {
    id: 'claude-desktop',
    name: 'Claude Desktop',
    icon: '🤖',
    description: 'Официальное приложение Anthropic. Конфиг в ~/Library/Application Support/Claude/claude_desktop_config.json (macOS) или %APPDATA%\\Claude\\claude_desktop_config.json (Windows).',
    configFormat: 'json',
    filePath: 'claude_desktop_config.json',
    buildConfig: (bin, proj) =>
      JSON.stringify(
        { mcpServers: { atlas: { command: bin, args: ['mcp', '--project', proj] } } },
        null,
        2,
      ),
    docsUrl: 'https://modelcontextprotocol.io/docs/quickstart',
  },
  {
    id: 'cursor',
    name: 'Cursor',
    icon: '💻',
    description: 'AI-редактор на базе VS Code. Конфиг в ~/.cursor/mcp.json или .cursor/mcp.json в проекте.',
    configFormat: 'json',
    filePath: '~/.cursor/mcp.json или .cursor/mcp.json',
    buildConfig: (bin, proj) =>
      JSON.stringify(
        { mcpServers: { atlas: { command: bin, args: ['mcp', '--project', proj] } } },
        null,
        2,
      ),
    docsUrl: 'https://docs.cursor.com/context/model-context-protocol',
  },
  {
    id: 'continue',
    name: 'Continue',
    icon: '🔷',
    description: 'Расширение для VS Code и JetBrains. Конфиг в ~/.continue/config.json.',
    configFormat: 'json',
    filePath: '~/.continue/config.json',
    buildConfig: (bin, proj) =>
      JSON.stringify(
        {
          mcpServers: {
            atlas: { command: bin, args: ['mcp', '--project', proj] },
          },
        },
        null,
        2,
      ),
    docsUrl: 'https://docs.continue.dev/customize/deep-dives/mcp',
  },
  {
    id: 'custom',
    name: 'Другой клиент',
    icon: '⚙️',
    description: 'Любой клиент с поддержкой MCP по stdio. Используйте стандартную конфигурацию mcpServers.',
    configFormat: 'json',
    filePath: '—',
    buildConfig: (bin, proj) =>
      JSON.stringify(
        { mcpServers: { atlas: { command: bin, args: ['mcp', '--project', proj] } } },
        null,
        2,
      ),
  },
]

export default function McpClients({ projectPath, binaryPath }: { projectPath: string; binaryPath: string }) {
  const [selected, setSelected] = useState<ClientId>('goose')
  const [copied, setCopied] = useState(false)

  const client = CLIENTS.find((c) => c.id === selected)!
  const config = client.buildConfig(binaryPath || 'atlas-cli', projectPath)

  const copy = async () => {
    await navigator.clipboard.writeText(config)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <div className="flex-1 overflow-auto p-6 space-y-5">
      <div className="flex items-center gap-3">
        <Plug className="w-6 h-6 text-accent-foreground" />
        <h1 className="text-2xl font-semibold">Подключение MCP-клиентов</h1>
      </div>
      <p className="text-xs text-muted-foreground max-w-2xl">
        Atlas работает как MCP-сервер по stdio. Любой клиент с поддержкой Model Context Protocol
        может подключиться к нему. Выберите приложение — покажем готовую конфигурацию.
      </p>

      {/* Выбор клиента */}
      <div className="flex flex-wrap gap-2">
        {CLIENTS.map((c) => (
          <button
            key={c.id}
            onClick={() => setSelected(c.id)}
            className={`flex items-center gap-2 px-3 py-2 rounded-lg text-xs border transition-colors ${
              selected === c.id
                ? 'bg-accent text-accent-foreground border-accent'
                : 'bg-muted border-border text-muted-foreground hover:bg-muted/70'
            }`}
          >
            <span className="text-base">{c.icon}</span>
            {c.name}
          </button>
        ))}
      </div>

      {/* Описание */}
      <div className="max-w-2xl bg-muted rounded-lg p-4 space-y-1">
        <div className="flex items-center gap-2">
          <span className="text-lg">{client.icon}</span>
          <span className="text-sm font-semibold">{client.name}</span>
          {client.docsUrl && (
            <a
              href={client.docsUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="ml-auto flex items-center gap-1 text-[11px] text-blue-500 hover:underline"
            >
              Документация <ExternalLink className="w-3 h-3" />
            </a>
          )}
        </div>
        <p className="text-xs text-muted-foreground">{client.description}</p>
      </div>

      {/* Конфигурация */}
      <div className="max-w-2xl space-y-2">
        <div className="flex items-center justify-between">
          <span className="text-xs font-semibold text-muted-foreground flex items-center gap-1.5">
            <Terminal className="w-3.5 h-3.5" />
            Конфигурация ({client.configFormat.toUpperCase()})
          </span>
          <button
            onClick={copy}
            className="flex items-center gap-1.5 text-xs bg-primary text-primary-foreground rounded-md px-3 py-1.5 font-medium"
          >
            {copied ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
            {copied ? 'Скопировано' : 'Скопировать'}
          </button>
        </div>
        <pre className="bg-[#1a191e] text-[#e0e0e0] rounded-lg p-4 text-[11px] font-mono overflow-x-auto whitespace-pre-wrap break-all">
          {config}
        </pre>
        {client.filePath !== '—' && (
          <p className="text-[11px] text-muted-foreground">
            Файл: <code className="bg-muted px-1 rounded">{client.filePath}</code>
          </p>
        )}
      </div>

      {/* Инструкция */}
      <div className="max-w-2xl bg-muted rounded-lg p-4 space-y-2">
        <h3 className="text-xs font-semibold">Как подключить</h3>
        <ol className="text-xs text-muted-foreground space-y-1 list-decimal list-inside">
          <li>Скопируйте конфигурацию выше.</li>
          <li>Откройте файл конфигурации {client.name} (путь указан выше).</li>
          <li>Добавьте блок <code className="bg-background px-1 rounded">mcpServers</code> или объедините с существующим.</li>
          <li>Перезапустите {client.name}.</li>
          <li>В чате с моделью попросите: «Покажи карту проекта через Atlas».</li>
        </ol>
      </div>

      {/* Список инструментов */}
      <div className="max-w-2xl bg-muted rounded-lg p-4 space-y-2">
        <h3 className="text-xs font-semibold">Доступные инструменты Atlas</h3>
        <div className="grid grid-cols-1 gap-1 text-[11px]">
          {[
            ['overview', 'Карта проекта; с path — файлы и символы каталога'],
            ['search', 'Поиск символов и файлов по имени, сигнатуре, докстрингу'],
            ['outline', 'Сигнатуры и строки всех символов одного файла'],
            ['read', 'Код символа или диапазона строк (mode: signature/doc/folded/full/contract)'],
            ['neighbors', 'Кто вызывает символ и что вызывает он; импорты файла'],
            ['context_for', 'Всё для работы с символом: код, зависимости, вызывающие'],
            ['status', 'Состояние индекса: свежесть, число узлов и связей'],
          ].map(([name, desc]) => (
            <div key={name} className="flex gap-3">
              <code className="bg-background px-1.5 rounded shrink-0">{name}</code>
              <span className="text-muted-foreground">{desc}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}