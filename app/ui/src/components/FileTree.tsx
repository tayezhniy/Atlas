import { useState, useEffect, useCallback } from 'react'
import { Folder, File, ChevronRight, ChevronDown, Code, FileText, Lock } from 'lucide-react'
import type { GraphNode } from '../types'

interface FileTreeNode {
  name: string
  type: 'dir' | 'file'
  path: string
  lang?: string
  children?: FileTreeNode[]
}

interface FileTreeProps {
  projectPath: string
  overview: { top: GraphNode[]; langs: Record<string, number>; files: number } | null
  onNodeSelect: (nodeId: string | null) => void
}

export default function FileTree({ projectPath, overview, onNodeSelect }: FileTreeProps) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set())
  const [treeData, setTreeData] = useState<FileTreeNode[]>([])
  const [loading, setLoading] = useState(false)

  // Построим дерево каталогов из топ-файлов обзора (имена путей)
  useEffect(() => {
    if (!overview) {
      setTreeData([])
      return
    }
    const paths = overview.top.map((n) => n.path)
    const tree = buildTree(paths)
    setTreeData(tree)
  }, [overview])

  const buildTree = (paths: string[]): FileTreeNode[] => {
    const root: FileTreeNode[] = []
    const nodeMap = new Map<string, FileTreeNode>()

    const getOrCreate = (path: string): FileTreeNode => {
      if (nodeMap.has(path)) return nodeMap.get(path)!
      const parts = path.split('/')
      const name = parts[parts.length - 1]
      const isFile = parts.length > 1 && !parts[parts.length - 1].includes('.') ? false : true
      // Определяем файл по наличию расширения
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

    // Сортируем: директории 먼저, потом файлы
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

  const toggleExpand = (path: string) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  }

  const renderNode = (node: FileTreeNode, depth: number) => {
    if (node.type === 'file') {
      const langIcon =
        node.lang === 'rust' ? <Code className="w-3 h-3" /> : <FileText className="w-3 h-3" />
      return (
        <div
          key={node.path}
          className="flex items-center gap-2 pl-4 py-1 text-xs hover:bg-muted cursor-pointer rounded transition-colors group"
          onClick={() => onNodeSelect(node.path)}
          title={node.path}
        >
          {langIcon}
          <span className="truncate">{node.name}</span>
          <span className="ml-auto text-[10px] text-muted-foreground">{node.lang}</span>
        </div>
      )
    }

    const isExpanded = expanded.has(node.path)
    return (
      <div key={node.path} className="pl-2">
        <div
          className="flex items-center gap-2 py-1 text-xs hover:bg-muted cursor-pointer rounded transition-colors group"
          onClick={() => toggleExpand(node.path)}
        >
          {isExpanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
          <Folder className="w-3 h-3" />
          <span className="font-medium truncate">{node.name}</span>
        </div>
        {isExpanded && node.children?.map((child) => renderNode(child, depth + 1))}
      </div>
    )
  }

  if (loading) {
    return (
      <div className="flex-1 p-4 text-center text-xs text-muted-foreground">
        Загрузка дерева файлов…
      </div>
    )
  }

  return (
    <div className="flex-1 overflow-y-auto p-1">
      {treeData.length === 0 && (
        <div className="p-4 text-center text-xs text-muted-foreground">
          Нет данных — проиндексируйте проект
        </div>
      )}
      {treeData.map((node) => renderNode(node, 0))}
      <div className="pt-4 pb-2">
        <div className="flex items-center gap-2 text-[10px] text-muted-foreground">
          <Lock className="w-3 h-3" />
          <span>Закрыто для нейросети</span>
        </div>
      </div>
    </div>
  )
}
