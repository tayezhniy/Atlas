// Типы ответов window.atlasAPI (preload -> main -> метод atlas/ui сервера)

export interface GraphNode {
  id: string
  name: string
  kind: string
  path: string
  lang?: string | null
  line_start?: number | null
  line_end?: number | null
  signature?: string | null
  doc?: string | null
  rank?: number
  tokens_est?: number
}

export type SymbolNode = GraphNode

export interface SearchResult extends GraphNode {
  score: number
}

export interface GraphEdge {
  source: string
  target: string
  kind: string
  confidence: string
}

export interface ProjectOverview {
  root: string
  files: number
  symbols: number
  langs: Record<string, number>
  top: GraphNode[]
  edges: GraphEdge[]
}

export interface EdgeInfo {
  kind: string
  direction: 'in' | 'out'
  target_id: string
  target_name: string
  target_path: string
  confidence: string
}

export interface FileContent {
  path: string
  lang?: string | null
  total: number
  start: number
  end: number
  lines: Array<{ line: number; text: string }>
}

export interface IndexStatus {
  indexed_at: string | null
  nodes: Record<string, number>
  edges: Record<string, number>
  calls: {
    by_tool: Array<{ tool: string; count: number; tokens_out: number }>
    recent: Array<{ time: string; tool: string; tokens_out: number }>
  }
}

export interface ReportData {
  project: {
    root: string
    generated_at: string
    files: number
    symbols: number
    languages: Record<string, number>
  }
  index_stats: {
    nodes_by_kind: Record<string, number>
    edges_by_kind: Record<string, number>
    total_nodes: number
    total_edges: number
  }
  top_symbols: Array<{
    name: string
    kind: string
    path: string
    rank: number
    tokens_est: number
    line_start?: number | null
    line_end?: number | null
  }>
  call_graph: {
    nodes: Array<{ id: string; name: string; kind: string; path: string; rank: number }>
    edges: GraphEdge[]
  }
  token_usage: { calls: number; tokens_out: number }
}

declare global {
  interface Window {
    atlasAPI: {
      ping: () => Promise<string>
      chooseProject: () => Promise<string | null>
      openProject: (projectPath: string) => Promise<{ ok: boolean; projectPath: string }>
      closeProject: (projectPath: string) => Promise<{ ok: boolean }>
      indexProject: (projectPath: string) => Promise<{ ok: boolean; output: string }>
      watch: (projectPath: string) => Promise<{ ok: boolean }>
      doctor: (projectPath: string) => Promise<{ ok: boolean; output: string }>
      overview: (projectPath: string) => Promise<ProjectOverview>
      status: (projectPath: string) => Promise<IndexStatus>
      search: (args: { projectPath: string; query: string; limit?: number }) => Promise<{
        query: string
        total: number
        items: SearchResult[]
      }>
      outline: (args: { projectPath: string; path: string }) => Promise<{
        path: string
        symbols: SymbolNode[]
      }>
      /** target — путь файла, `путь::имя` или идентификатор узла */
      read: (args: {
        projectPath: string
        target: string
        mode?: 'signature' | 'doc' | 'folded' | 'full'
      }) => Promise<FileContent>
      neighbors: (args: { projectPath: string; target: string }) => Promise<{
        node: GraphNode
        outgoing: EdgeInfo[]
        incoming: EdgeInfo[]
      }>
      mcpConfig: (projectPath: string) => Promise<string>
      report: {
        (args: { projectPath: string; format: 'json' }): Promise<ReportData>
        (args: { projectPath: string; format: 'markdown' }): Promise<string>
      }
      settings: {
        all: (projectPath: string) => Promise<{ ok: boolean; settings: Record<string, any> }>
        set: (projectPath: string, key: string, value: string) => Promise<{ ok: boolean }>
      }
    }
  }
}

export {}
