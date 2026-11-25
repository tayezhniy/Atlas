// Типы для API Atlas, доступного через window.atlasAPI (Electron preload)

// Результат поиска
export interface SearchResult {
  id: string
  kind: string
  path: string
  name: string
  lang?: string
  line_start?: number
  line_end?: number
  signature?: string
  doc?: string
  score: number
  snippet?: string
}

// Символ в файле (outline)
export interface SymbolNode {
  id: string
  kind: string
  name: string
  path: string
  line_start?: number
  line_end?: number
  signature?: string
  doc?: string
}

// Связь между узлами (неighbours)
export interface EdgeInfo {
  kind: string
  direction: 'in' | 'out'
  target_id: string
  target_name: string
  target_path: string
  confidence: string
}

// Узел графа
export interface GraphNode {
  id: string
  name: string
  kind: string
  path: string
  rank?: number
  tokens_est?: number
}

// Обзор проекта
export interface ProjectOverview {
  root: string
  files: number
  langs: Record<string, number>
  top: GraphNode[]
}

// Состояние индекса (doctor)
export interface DoctorResult {
  ok: boolean
  result?: any
  output?: string
}

// Глобальное API, инжектируемое preload-скриптом
declare global {
  interface Window {
    atlasAPI: {
      ping: () => Promise<string>
      openProject: (projectPath: string) => Promise<{ ok: boolean; projectPath: string }>
      indexProject: (projectPath: string) => Promise<{ ok: boolean; output: string }>
      doctor: (projectPath: string) => Promise<DoctorResult>
      search: (args: {
        projectPath: string
        query: string
        limit?: number
      }) => Promise<{ query: string; total: number; items: SearchResult[] }>
      overview: (projectPath: string, path?: string) => Promise<ProjectOverview>
      outline: (args: { projectPath: string; path: string }) => Promise<{
        path: string
        symbols: SymbolNode[]
      }>
      read: (args: {
        projectPath: string
        path: string
        start_line?: number
        end_line?: number
        mode?: 'signature' | 'doc' | 'folded' | 'full'
      }) => Promise<{
        path: string
        total: number
        start: number
        end: number
        lines: Array<{ line: number; text: string }>
      }>
      neighbors: (args: { projectPath: string; target: string }) => Promise<{
        node: { id: string; name: string; kind: string; path: string }
        outgoing: EdgeInfo[]
        incoming: EdgeInfo[]
      }>
      contextFor: (args: {
        projectPath: string
        symbol: string
        path?: string
        max_tokens?: number
      }) => Promise<{
        symbol: { name: string; kind: string; path: string }
        outgoing: EdgeInfo[]
        incoming: EdgeInfo[]
        tests: EdgeInfo[]
        code: string
      }>
      status: (projectPath: string) => Promise<any>
      tree: (args: { projectPath: string; path?: string }) => Promise<ProjectOverview>
      callLog: (projectPath: string, options?: any) => Promise<any>
      settings: {
        get: (projectPath: string, key: string) => Promise<any>
        set: (projectPath: string, key: string, value: string) => Promise<any>
        all: (projectPath: string) => Promise<any>
      }
      report: (projectPath: string, options?: any) => Promise<any>
      closeProject: (projectPath: string) => Promise<{ ok: boolean }>
    }
  }
}

export {}
