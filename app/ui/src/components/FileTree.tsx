import { useState, useEffect, useCallback } from 'react'
import {
  Folder, ChevronRight, ChevronDown, Code, FileText,
  Eye, EyeOff, Copy, ExternalLink
} from 'lucide-react'
import type { GraphNode } from '../types'

interface FileTreeNode {
  name: string
  type: 'dir' | 'file'
  path: string
  lang?: string
  children?: FileTreeNode[]
  symbols?: SymbolNode[]
  symbolsLoaded?: boolean
  symbolsLoading?: boolean
}

interface SymbolNode {
  id: string
  name: string
  kind: string
  line_start?: number
  line_end?: number
  signature?: string
}

interface FileTreeProps {
  projectPath: string
  overview: { top: GraphNode[]; langs: Record<string, number>; files: number } | null
  onNodeSelect: (nodeId: string | null, nodeType?: 'file' | 'symbol') => void
  onGraphHighlight: (nodeId: string) => void
  selectedNode: string | null
}

interface ContextMenuState {
  x: number
  y: number
  node: FileTreeNode | null
  visible: boolean
}

export default function FileTree({
  projectPath,
  overview,
  onNodeSelect,
  onGraphHighlight,
  selectedNode,
}: FileTreeProps) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set())
  const [treeData, setTreeData] = useState<FileTreeNode[]>([])
  const [loading, setLoading] = useState(false)
  const [contextMenu, setContextMenu] = useState<ContextMenuState>({ x: 0, y: 0, node: null, visible: false })

  // Закрытие контекстного меню при клике вне
  useEffect(() => {
    const handleClick = () => setContextMenu(prev => ({ ...prev, visible: false }))
    document.addEventListener('click', handleClick)
    return () => document.removeEventListener('click', handleClick)
  }, [])

  // Построим полное дерево каталогов через MCP tree tool
  useEffect(() => {
    if (!projectPath || !window.atlasAPI) {
      setTreeData([])
      return
    }
    loadFullTree()
  }, [projectPath])

  const loadFullTree = async () => {
    setLoading(true)
    try {
      const result = await window.atlasAPI!.tree({ projectPath, path: '' })
      const paths = result.top.map((n: GraphNode) => n.path)
      const tree = buildTree(paths)
      setTreeData(tree)
    } catch {
      // Fallback: строим из overview
      if (overview) {
        const paths = overview.top.map(n => n.path)
        const tree = buildTree(paths)
        setTreeData(tree)
      }
    } finally {
      setLoading(false)
    }
  }

  const buildTree = (paths: string[]): FileTreeNode[] => {
    const root: FileTreeNode[] = []
    const nodeMap = new Map<string, FileTreeNode>()

    const getOrCreate = (path: string): FileTreeNode => {
      if (nodeMap.has(path)) return nodeMap.get(path)!
      const parts = path.split('/')
      const name = parts[parts.length - 1]
      const last = parts[parts.length - 1]
      const isFileNode = last.includes('.') && !last.startsWith('.')
      const node: FileTreeNode = {
        name,
        type: isFileNode ? 'file' : 'dir',
        path,
        lang: isFileNode ? last.split('.').pop() : undefined,
      }
      nodeMap.set(path, node)
      return node
    }

    paths.forEach((p) => {
      const parts = p.split('/')
      let parent: FileTreeNode | null = null
      for (let i = 0; i < parts.length; i++) {
        const prefix = parts.slice(0, i + 1).join('/')
        const node = getOrCreate(prefix)
        if (i === 0) {
          if (!root.find((r) => r.path === prefix)) root.push(node)
        } else if (parent) {
          if (!parent.children) parent.children = []
          if (!parent.children.find((c) => c.path === prefix)) parent.children.push(node)
        }
        parent = node
      }
    })

    const sortNodes = (nodes: FileTreeNode[]) => {
      nodes.sort((a, b) => {
        if (a.type !== b.type) return a.type === 'dir' ? -1 : 1
        return a.name.localeCompare(b.name)
      })
      nodes.forEach((n) => {
        if (n.children) sortNodes(n.children)
      })
    }
    sortNodes(root)
    return root
  }

  const toggleExpand = useCallback((path: string) => {
    setExpanded(prev => {
      const next = new Set(prev)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  }, [])

  const loadFileSymbols = async (node: FileTreeNode) => {
    if (node.symbolsLoaded || node.symbolsLoading || !window.atlasAPI) return
    setTreeData(prev => updateNode(prev, node.path, { symbolsLoading: true }))
    try {
      const result = await window.atlasAPI.outline({ projectPath, path: node.path })
      const symbols: SymbolNode[] = result.symbols.map(s => ({
        id: s.id,
        name: s.name,
        kind: s.kind,
        line_start: s.line_start,
        line_end: s.line_end,
        signature: s.signature,
      }))
      setTreeData(prev => updateNode(prev, node.path, { symbols, symbolsLoaded: true, symbolsLoading: false }))
    } catch {
      setTreeData(prev => updateNode(prev, node.path, { symbolsLoading: false }))
    }
  }

  const updateNode = (nodes: FileTreeNode[], targetPath: string, updates: Partial<FileTreeNode>): FileTreeNode[] => {
    return nodes.map(node => {
      if (node.path === targetPath) return { ...node, ...updates }
      if (node.children) return { ...node, children: updateNode(node.children, targetPath, updates) }
      return node
    })
  }

  const handleNodeClick = (node: FileTreeNode, e: React.MouseEvent) => {
    if (e.button === 2) return // правый клик — контекстное меню
    if (node.type === 'dir') {
      toggleExpand(node.path)
    } else {
      onNodeSelect(node.path, 'file')
    }
  }

  const handleSymbolClick = (symbol: SymbolNode, nodePath: string) => {
    const symbolId = `${nodePath}::${symbol.name}`
    onNodeSelect(symbolId, 'symbol')
    onGraphHighlight(symbolId)
  }

  const handleContextMenu = (e: React.MouseEvent, node: FileTreeNode) => {
    e.preventDefault()
    e.stopPropagation()
    setContextMenu({ x: e.clientX, y: e.clientY, node, visible: true })
  }

  const handleContextAction = (action: string, node: FileTreeNode) => {
    setContextMenu(prev => ({ ...prev, visible: false }))
    switch (action) {
      case 'copy-path':
        navigator.clipboard.writeText(node.path)
        break
      case 'open-in-graph':
        onGraphHighlight(node.path)
        break
      case 'open-external':
        // TODO: открыть в внешнем редакторе
        break
      case 'exclude':
        // TODO: добавить в исключения
        break
    }
  }

  const renderNode = (node: FileTreeNode, depth: number) => {
    const isSelected = selectedNode === node.path
    const isExpanded = expanded.has(node.path)

    if (node.type === 'file') {
      const langIcon = node.lang === 'rust' ? <Code className="w-3 h-3" /> : <FileText className="w-3 h-3" />
      const showSymbols = isExpanded && node.symbols && node.symbols.length > 0
      const loadingSymbols = isExpanded && node.symbolsLoading

      return (
        <div key={node.path} className="pl-2">
          <div
            className={`flex items-center gap-2 pl-4 py-1 text-xs hover:bg-muted cursor-pointer rounded transition-colors group ${
              isSelected ? 'bg-accent text-accent-foreground' : ''
            }`}
            onClick={e => handleNodeClick(node, e as any)}
            onContextMenu={e => handleContextMenu(e as any, node)}
            onDoubleClick={() => loadFileSymbols(node)}
            title={node.path}
          >
            {node.symbols !== undefined ? (
              isExpanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />
            ) : (
              <span className="w-3 h-3" />
            )}
            {langIcon}
            <span className="truncate flex-1">{node.name}</span>
            {node.lang && <span className="ml-auto text-[10px] text-muted-foreground">{node.lang}</span>}
            {loadingSymbols && <span className="text-[10px] text-blue-500 animate-pulse">загрузка…</span>}
          </div>
          {showSymbols && (
            <div className="pl-6 border-l border-muted/50 ml-3">
              {node.symbols!.map(sym => (
                <div
                  key={sym.id}
                  className="flex items-center gap-2 pl-2 py-1 text-xs hover:bg-muted/50 cursor-pointer rounded transition-colors"
                  onClick={() => handleSymbolClick(sym, node.path)}
                  onContextMenu={e => {
                    e.preventDefault()
                    e.stopPropagation()
                    setContextMenu({ x: e.clientX, y: e.clientY, node: { ...node, path: `${node.path}::${sym.name}` }, visible: true })
                  }}
                  title={`${sym.kind}: ${sym.signature || sym.name}`}
                >
                  <span className="w-3 h-3 text-muted-foreground">{getSymbolIcon(sym.kind)}</span>
                  <span className="truncate flex-1">{sym.name}</span>
                  {sym.line_start && <span className="text-[10px] text-muted-foreground">L{sym.line_start}</span>}
                </div>
              ))}
              {node.symbols!.length === 0 && (
                <div className="pl-5 py-1 text-[10px] text-muted-foreground">Символов не найдено</div>
              )}
            </div>
          )}
          {isExpanded && !node.symbolsLoaded && !node.symbolsLoading && node.type === 'file' && (
            <div className="pl-6 py-1 text-center text-[10px] text-muted-foreground" onClick={() => loadFileSymbols(node)}>
              Двойной клик — загрузить символы
            </div>
          )}
        </div>
      )
    }

    return (
      <div key={node.path} className="pl-2">
        <div
          className={`flex items-center gap-2 py-1 text-xs hover:bg-muted cursor-pointer rounded transition-colors group ${
            isSelected ? 'bg-accent text-accent-foreground' : ''
          }`}
          onClick={() => toggleExpand(node.path)}
          onContextMenu={e => handleContextMenu(e as any, node)}
          title={node.path}
        >
          {isExpanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
          <Folder className="w-3 h-3" />
          <span className="font-medium truncate">{node.name}</span>
        </div>
        {isExpanded && node.children?.map(child => renderNode(child, depth + 1))}
      </div>
    )
  }

  const getSymbolIcon = (kind: string) => {
    switch (kind) {
      case 'function': return 'ƒ'
      case 'method': return 'm'
      case 'class': return 'C'
      case 'interface': return 'I'
      case 'type': return 'T'
      case 'constant': return 'C'
      case 'module': return 'M'
      default: return '•'
    }
  }

  if (loading) {
    return (
      <div className="flex-1 p-4 text-center text-xs text-muted-foreground">
        Загрузка дерева файлов…
      </div>
    )
  }

  return (
    <>
      <div className="flex-1 overflow-y-auto p-1">
        {treeData.length === 0 && (
          <div className="p-4 text-center text-xs text-muted-foreground">
            Нет данных — проиндексируйте проект
          </div>
        )}
        {treeData.map(node => renderNode(node, 0))}
        <div className="pt-4 pb-2 border-t">
          <div className="flex items-center gap-2 text-[10px] text-muted-foreground">
            <span className="w-3 h-3 rounded-full bg-muted"></span>
            <span>ПКМ — контекстное меню</span>
          </div>
        </div>
      </div>

      {/* Контекстное меню */}
      {contextMenu.visible && contextMenu.node && (
        <div
          className="fixed z-50 bg-background border shadow-lg rounded-md py-1 min-w-[180px] text-xs"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          role="menu"
        >
          <div className="px-3 py-1.5 text-muted-foreground border-b capitalize">{contextMenu.node.type}</div>
          <div className="px-2 py-0.5 text-[10px] text-muted-foreground truncate" title={contextMenu.node.path}>
            {contextMenu.node.path}
          </div>
          <div className="border-y my-1" />
          <button
            className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-accent text-left"
            onClick={() => handleContextAction('copy-path', contextMenu.node!)}
            role="menuitem"
          >
            <Copy className="w-3 h-3" />
            Копировать путь
          </button>
          <button
            className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-accent text-left"
            onClick={() => handleContextAction('open-in-graph', contextMenu.node!)}
            role="menuitem"
          >
            <ExternalLink className="w-3 h-3" />
            Подсветить на графе
          </button>
          <button
            className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-accent text-left"
            onClick={() => handleContextAction('open-external', contextMenu.node!)}
            role="menuitem"
          >
            <Eye className="w-3 h-3" />
            Открыть во внешнем редакторе
          </button>
          <button
            className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-accent text-left text-red-500"
            onClick={() => handleContextAction('exclude', contextMenu.node!)}
            role="menuitem"
          >
            <EyeOff className="w-3 h-3" />
            Исключить из индекса
          </button>
        </div>
      )}
    </>
  )
}