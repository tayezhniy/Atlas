import { useState } from 'react'
import { Folder, File, ChevronRight, ChevronDown, Code, FileText, Lock } from 'lucide-react'

interface FileTreeProps {
  projectPath: string
  onNodeSelect: (nodeId: string | null) => void
}

export default function FileTree({ projectPath, onNodeSelect }: FileTreeProps) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set())

  // Моковые данные — в будущем будут загружаться из API
  const treeData = [
    {
      name: 'src',
      type: 'dir' as const,
      children: [
        { name: 'main.rs', type: 'file' as const, lang: 'rust' },
        { name: 'lib.rs', type: 'file' as const, lang: 'rust' },
        { name: 'api', type: 'dir' as const, children: [
          { name: 'handlers.rs', type: 'file' as const, lang: 'rust' },
          { name: 'middleware.rs', type: 'file' as const, lang: 'rust' },
        ]},
        { name: 'core', type: 'dir' as const, children: [
          { name: 'mod.rs', type: 'file' as const, lang: 'rust' },
          { name: 'types.rs', type: 'file' as const, lang: 'rust' },
        ]},
      ],
    },
    {
      name: 'tests',
      type: 'dir' as const,
      children: [
        { name: 'integration_test.rs', type: 'file' as const, lang: 'rust' },
      ],
    },
    {
      name: 'Cargo.toml',
      type: 'file' as const,
      lang: 'toml',
    },
    {
      name: 'README.md',
      type: 'file' as const,
      lang: 'markdown',
    },
  ]

  const toggleExpand = (path: string) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  }

  const renderNode = (node: any, depth: number) => {
    if (node.type === 'file') {
      const langIcon = node.lang === 'rust' ? <Code className="w-3 h-3" /> : <FileText className="w-3 h-3" />
      return (
        <div
          key={node.name}
          className="flex items-center gap-2 pl-4 py-1 text-xs hover:bg-muted cursor-pointer rounded transition-colors group"
          onClick={() => onNodeSelect(node.name)}
        >
          {langIcon}
          <span className="truncate">{node.name}</span>
          <span className="ml-auto text-[10px] text-muted-foreground">{node.lang}</span>
        </div>
      )
    }

    const isExpanded = expanded.has(node.name)
    return (
      <div key={node.name} className="pl-2">
        <div
          className="flex items-center gap-2 py-1 text-xs hover:bg-muted cursor-pointer rounded transition-colors group"
          onClick={() => toggleExpand(node.name)}
        >
          {isExpanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
          <Folder className="w-3 h-3" />
          <span className="font-medium">{node.name}</span>
        </div>
        {isExpanded && node.children?.map((child: any) => renderNode(child, depth + 1))}
      </div>
    )
  }

  return (
    <div className="flex-1 overflow-y-auto p-1">
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