import { useEffect, useMemo, useState } from 'react'
import { ChevronDown, ChevronRight, Folder } from 'lucide-react'
import type { GraphNode } from '../types'
import { badge } from '../lib'

interface TreeNode {
  name: string
  path: string
  isFile: boolean
  children: TreeNode[]
}

interface FileTreeProps {
  projectName: string
  nodes: GraphNode[]
  selected: string | null
  onSelect: (path: string) => void
  onOpen: (path: string) => void
}

function buildTree(paths: string[]): TreeNode[] {
  const root: TreeNode = { name: '', path: '', isFile: false, children: [] }
  const seen = new Map<string, TreeNode>([['', root]])
  for (const path of paths) {
    const parts = path.split('/')
    let parent = root
    parts.forEach((name, i) => {
      const key = parts.slice(0, i + 1).join('/')
      let node = seen.get(key)
      if (!node) {
        node = { name, path: key, isFile: i === parts.length - 1, children: [] }
        seen.set(key, node)
        parent.children.push(node)
      }
      parent = node
    })
  }
  const sort = (list: TreeNode[]) => {
    list.sort((a, b) => Number(a.isFile) - Number(b.isFile) || a.name.localeCompare(b.name))
    list.forEach((n) => sort(n.children))
  }
  sort(root.children)
  return root.children
}

export default function FileTree({ projectName, nodes, selected, onSelect, onOpen }: FileTreeProps) {
  const tree = useMemo(() => buildTree(nodes.map((n) => n.path)), [nodes])
  const [open, setOpen] = useState<Set<string>>(new Set(['']))
  const selectedPath = selected?.split('::')[0] ?? null

  // Раскрываем каталоги до выбранного файла
  useEffect(() => {
    if (!selectedPath) return
    const parts = selectedPath.split('/')
    setOpen((prev) => new Set([...prev, '', ...parts.slice(0, -1).map((_, i) => parts.slice(0, i + 1).join('/'))]))
  }, [selectedPath])

  const toggle = (path: string) =>
    setOpen((prev) => {
      const next = new Set(prev)
      if (!next.delete(path)) next.add(path)
      return next
    })

  const render = (node: TreeNode, depth: number): JSX.Element => {
    const expanded = open.has(node.path)
    const active = node.isFile && node.path === selectedPath
    return (
      <div key={node.path || '/'}>
        <button
          onClick={() => (node.isFile ? onSelect(node.path) : toggle(node.path))}
          onDoubleClick={() => node.isFile && onOpen(node.path)}
          className={`w-full flex items-center gap-2 h-[30px] rounded-md pr-2 text-xs text-left ${
            active ? 'bg-accent text-accent-foreground' : node.isFile ? 'text-muted-foreground hover:bg-muted' : 'hover:bg-muted'
          }`}
          style={{ paddingLeft: 8 + depth * 15 }}
          title={node.path || projectName}
        >
          {node.isFile ? (
            <span className="w-7 shrink-0 text-[9px] font-semibold text-blue-600 ml-5">{badge(node.path)}</span>
          ) : (
            <>
              {expanded ? <ChevronDown className="w-3 h-3 shrink-0" /> : <ChevronRight className="w-3 h-3 shrink-0" />}
              <Folder className="w-3.5 h-3.5 shrink-0" />
            </>
          )}
          <span className="truncate">{node.name || projectName}</span>
        </button>
        {!node.isFile && expanded && node.children.map((c) => render(c, depth + 1))}
      </div>
    )
  }

  return <div>{render({ name: '', path: '', isFile: false, children: tree }, 0)}</div>
}
