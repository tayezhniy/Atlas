import type { GraphEdge, GraphNode } from './types'

/** Цвета групп (каталогов) на графе — из макета. */
export const GROUP_COLORS = ['#a691cd', '#85afcf', '#88b9a2', '#c6b080']
export const OTHER_COLOR = '#777583'

export const fileName = (path: string) => path.split('/').pop() ?? path

export const dirName = (path: string) => path.split('/').slice(0, -1).join('/')

export const formatNumber = (n: number) => n.toLocaleString('ru-RU')

export function badge(path: string): string {
  const ext = path.includes('.') ? path.split('.').pop()!.toLowerCase() : ''
  if (ext === 'json') return '{}'
  if (ext === 'md') return 'M↓'
  return ext.slice(0, 3).toUpperCase() || '·'
}

/** Группа файла — его каталог; четыре самых крупных получают свой цвет. */
export function groupsOf(nodes: GraphNode[]) {
  const label = (path: string) => dirName(path).split('/').pop() || 'корень'
  const count = new Map<string, number>()
  for (const n of nodes) count.set(label(n.path), (count.get(label(n.path)) ?? 0) + 1)
  const top = [...count.entries()].sort((a, b) => b[1] - a[1]).slice(0, GROUP_COLORS.length).map(([g]) => g)
  const color = (path: string) => GROUP_COLORS[top.indexOf(label(path))] ?? OTHER_COLOR
  return { top, color }
}

/** Связи выбранного файла: что он импортирует и кто его использует. */
export function fileLinks(path: string, nodes: GraphNode[], edges: GraphEdge[]) {
  const byId = new Map(nodes.map((n) => [n.id, n.path]))
  const imports = new Set<string>()
  const usedBy = new Set<string>()
  const all = new Set<string>()
  for (const e of edges) {
    const src = byId.get(e.source)
    const dst = byId.get(e.target)
    if (!src || !dst) continue
    if (src === path) {
      all.add(dst)
      if (e.kind === 'imports') imports.add(dst)
    }
    if (dst === path) {
      all.add(src)
      usedBy.add(src)
    }
  }
  return { imports: [...imports], usedBy: [...usedBy], total: all.size }
}

export function timeAgo(value: string | null | undefined): string {
  if (!value) return 'ещё не индексировался'
  const date = new Date(/[TZ]/.test(value) ? value : value.replace(' ', 'T') + 'Z')
  if (Number.isNaN(date.getTime())) return value
  const seconds = Math.max(0, Math.round((Date.now() - date.getTime()) / 1000))
  if (seconds < 60) return 'только что'
  if (seconds < 3600) return `${Math.floor(seconds / 60)} мин назад`
  if (seconds < 86400) return `${Math.floor(seconds / 3600)} ч назад`
  return date.toLocaleDateString('ru-RU')
}
