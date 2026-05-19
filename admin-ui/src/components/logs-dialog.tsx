import { useMemo, useState } from 'react'
import { toast } from 'sonner'
import {
  Activity,
  AlertCircle,
  CheckCircle2,
  ChevronDown,
  Copy,
  FileText,
  Loader2,
  Pause,
  Play,
  RefreshCw,
  Search,
  Trash2,
  XCircle,
  Zap,
} from 'lucide-react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Switch } from '@/components/ui/switch'
import { extractErrorMessage } from '@/lib/utils'
import { useLogs, useClearLogs } from '@/hooks/use-logs'
import type { LogEntry, LogsQueryParams } from '@/types/api'

interface LogsDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

type KindFilter = 'all' | 'generic' | 'model_call'

const LEVEL_OPTIONS: { value: string; label: string }[] = [
  { value: 'all', label: '全部' },
  { value: 'INFO', label: 'INFO' },
  { value: 'WARN', label: 'WARN' },
  { value: 'ERROR', label: 'ERROR' },
]

function formatTime(ts: number): string {
  const d = new Date(ts)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${String(d.getMilliseconds()).padStart(3, '0')}`
}

function levelClasses(level: string): { text: string; bar: string; bg: string } {
  const u = level.toUpperCase()
  if (u === 'ERROR') {
    return { text: 'text-red-600 dark:text-red-400', bar: 'bg-red-500', bg: 'bg-red-50/40 dark:bg-red-950/10' }
  }
  if (u === 'WARN') {
    return { text: 'text-amber-600 dark:text-amber-400', bar: 'bg-amber-500', bg: '' }
  }
  return { text: 'text-blue-600 dark:text-blue-400', bar: 'bg-blue-400/60', bg: '' }
}

function statusInfo(status: number): { color: string; icon: React.ReactNode; label: string; bar: string } {
  if (status === 0) {
    return { color: 'text-red-600', icon: <XCircle className="h-3 w-3" />, label: 'NET', bar: 'bg-red-500' }
  }
  if (status >= 400) {
    return { color: 'text-red-600', icon: <XCircle className="h-3 w-3" />, label: String(status), bar: 'bg-red-500' }
  }
  return { color: 'text-green-600', icon: <CheckCircle2 className="h-3 w-3" />, label: String(status), bar: 'bg-green-500' }
}

function copyToClipboard(text: string) {
  try {
    navigator.clipboard.writeText(text)
    toast.success('已复制到剪贴板')
  } catch {
    toast.error('复制失败')
  }
}

export function LogsDialog({ open, onOpenChange }: LogsDialogProps) {
  const [kindFilter, setKindFilter] = useState<KindFilter>('all')
  const [levelFilter, setLevelFilter] = useState<string>('all')
  const [searchTerm, setSearchTerm] = useState('')
  const [onlyFailed, setOnlyFailed] = useState(false)
  const [autoRefresh, setAutoRefresh] = useState(true)
  const [expanded, setExpanded] = useState<Set<number>>(new Set())

  const params: LogsQueryParams = useMemo(() => {
    const p: LogsQueryParams = { limit: 500 }
    if (kindFilter !== 'all') p.kind = kindFilter
    if (levelFilter !== 'all') p.levels = levelFilter
    if (searchTerm.trim()) p.q = searchTerm.trim()
    if (onlyFailed) p.onlyFailed = true
    return p
  }, [kindFilter, levelFilter, searchTerm, onlyFailed])

  const { data, isLoading, error, refetch } = useLogs(params, open, autoRefresh)
  const { mutateAsync: clearMutate, isPending: isClearing } = useClearLogs()

  const handleClear = async () => {
    if (!confirm('清空日志缓冲？此操作无法撤销。')) return
    try {
      await clearMutate()
      toast.success('日志缓冲已清空')
      setExpanded(new Set())
    } catch (e) {
      toast.error('清空失败: ' + extractErrorMessage(e))
    }
  }

  const toggleExpand = (seq: number) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(seq)) {
        next.delete(seq)
      } else {
        next.add(seq)
      }
      return next
    })
  }

  const stats = data?.stats
  const successRate = stats && stats.total > 0 ? (stats.success / stats.total) * 100 : null
  const hasActiveFilter = kindFilter !== 'all' || levelFilter !== 'all' || searchTerm.trim() !== '' || onlyFailed
  const filteredCount = data?.entries.length ?? 0

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-5xl">
        <DialogHeader className="pb-2">
          <DialogTitle className="flex items-center gap-2 text-base">
            <div className="h-7 w-7 rounded-md bg-primary/10 flex items-center justify-center">
              <FileText className="h-4 w-4 text-primary" />
            </div>
            <span>日志查看</span>
            {autoRefresh && (
              <span className="ml-1 inline-flex items-center gap-1 text-xs font-normal text-green-600">
                <span className="h-1.5 w-1.5 rounded-full bg-green-500 animate-pulse" />
                实时
              </span>
            )}
            <span className="flex-1" />
            <span className="text-xs font-normal text-muted-foreground">
              {hasActiveFilter && (
                <span className="font-mono">{filteredCount}/</span>
              )}
              <span className="font-mono">{data?.totalBuffered ?? 0}</span>
              <span className="text-muted-foreground/60"> / {data?.capacity ?? 0}</span>
            </span>
          </DialogTitle>
          <DialogDescription className="text-xs mt-1">
            内存环形缓冲，重启丢失
          </DialogDescription>
        </DialogHeader>

        {/* 统计条 - 最近 5 分钟 ModelCall */}
        {stats && (
          <div className="grid grid-cols-5 gap-2 text-xs">
            <StatCard
              icon={<Activity className="h-3 w-3" />}
              label="调用"
              value={stats.total}
              hint="5 分钟"
            />
            <StatCard
              icon={<CheckCircle2 className="h-3 w-3" />}
              label="成功"
              value={stats.success}
              tone="success"
            />
            <StatCard
              icon={<XCircle className="h-3 w-3" />}
              label="失败"
              value={stats.failed}
              tone={stats.failed > 0 ? 'danger' : 'default'}
            />
            <StatCard
              icon={<Zap className="h-3 w-3" />}
              label="成功率"
              value={successRate != null ? `${successRate.toFixed(1)}%` : '—'}
              tone={successRate != null && successRate < 90 ? 'warn' : 'success'}
            />
            <StatCard
              icon={<RefreshCw className="h-3 w-3" />}
              label="P95"
              value={stats.total > 0 ? `${stats.p95Ms}ms` : '—'}
              hint={stats.total > 0 ? `平均 ${stats.avgMs}ms` : undefined}
            />
          </div>
        )}

        {/* 工具栏 */}
        <div className="space-y-2">
          <div className="flex items-center gap-2 flex-wrap">
            <FilterSegment
              options={[
                { value: 'all', label: '全部' },
                { value: 'generic', label: '系统' },
                { value: 'model_call', label: '模型调用' },
              ]}
              value={kindFilter}
              onChange={(v) => setKindFilter(v as KindFilter)}
            />
            <FilterSegment
              options={LEVEL_OPTIONS}
              value={levelFilter}
              onChange={setLevelFilter}
            />
            <div className="relative flex-1 min-w-[200px]">
              <Search className="h-3.5 w-3.5 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground" />
              <input
                type="text"
                placeholder="搜索消息 / target / 字段"
                value={searchTerm}
                onChange={(e) => setSearchTerm(e.target.value)}
                className="w-full h-8 pl-8 pr-2 text-xs border rounded-md bg-background focus:outline-none focus:ring-2 focus:ring-primary/30"
              />
            </div>
            <label className="inline-flex items-center gap-1.5 text-xs text-muted-foreground cursor-pointer select-none px-2 h-8 rounded-md border bg-muted/30 hover:bg-muted/50">
              <Switch
                checked={onlyFailed}
                onCheckedChange={setOnlyFailed}
                className="scale-75"
              />
              仅失败
            </label>
          </div>

          <div className="flex items-center gap-1 text-xs">
            <Button
              size="sm"
              variant="ghost"
              className="h-7 text-xs"
              onClick={() => setAutoRefresh((v) => !v)}
              title={autoRefresh ? '暂停自动刷新' : '开启自动刷新（每 3s）'}
            >
              {autoRefresh ? <Pause className="h-3 w-3 mr-1" /> : <Play className="h-3 w-3 mr-1" />}
              {autoRefresh ? '暂停' : '继续'}
            </Button>
            <Button
              size="sm"
              variant="ghost"
              className="h-7 text-xs"
              onClick={() => refetch()}
              disabled={isLoading}
            >
              <RefreshCw className={`h-3 w-3 mr-1 ${isLoading ? 'animate-spin' : ''}`} />
              立即刷新
            </Button>
            {expanded.size > 0 && (
              <Button
                size="sm"
                variant="ghost"
                className="h-7 text-xs"
                onClick={() => setExpanded(new Set())}
              >
                收起全部 ({expanded.size})
              </Button>
            )}
            <div className="flex-1" />
            <Button
              size="sm"
              variant="ghost"
              className="h-7 text-xs text-destructive hover:text-destructive hover:bg-destructive/10"
              onClick={handleClear}
              disabled={isClearing}
            >
              <Trash2 className="h-3 w-3 mr-1" /> 清空缓冲
            </Button>
          </div>
        </div>

        {/* 日志列表 */}
        <div className="border rounded-md max-h-[480px] overflow-y-auto bg-background">
          {error ? (
            <div className="p-8 text-center text-sm text-red-500">
              <AlertCircle className="h-6 w-6 mx-auto mb-2" />
              加载失败：{extractErrorMessage(error)}
            </div>
          ) : isLoading && !data ? (
            <div className="p-8 text-center text-sm text-muted-foreground">
              <Loader2 className="h-6 w-6 mx-auto mb-2 animate-spin" />
              加载中...
            </div>
          ) : !data?.entries.length ? (
            <div className="p-8 text-center text-sm text-muted-foreground">
              <FileText className="h-6 w-6 mx-auto mb-2 opacity-30" />
              {hasActiveFilter ? (
                <>
                  没有匹配当前过滤条件的日志
                  <div className="mt-2">
                    <Button
                      size="sm"
                      variant="outline"
                      className="h-7 text-xs"
                      onClick={() => {
                        setKindFilter('all')
                        setLevelFilter('all')
                        setSearchTerm('')
                        setOnlyFailed(false)
                      }}
                    >
                      清除过滤
                    </Button>
                  </div>
                </>
              ) : (
                <>暂无日志</>
              )}
            </div>
          ) : (
            <div className="divide-y divide-border/50">
              {data.entries.map((entry) => (
                <LogRow
                  key={entry.seq}
                  entry={entry}
                  expanded={expanded.has(entry.seq)}
                  onToggle={() => toggleExpand(entry.seq)}
                />
              ))}
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}

function StatCard({
  icon,
  label,
  value,
  hint,
  tone = 'default',
}: {
  icon: React.ReactNode
  label: string
  value: number | string
  hint?: string
  tone?: 'default' | 'success' | 'warn' | 'danger'
}) {
  const toneClass =
    tone === 'success'
      ? 'text-green-600 dark:text-green-400'
      : tone === 'warn'
        ? 'text-amber-600 dark:text-amber-400'
        : tone === 'danger'
          ? 'text-red-600 dark:text-red-400'
          : 'text-foreground'
  return (
    <div className="px-3 py-2 rounded-md border bg-card">
      <div className="text-[10px] text-muted-foreground flex items-center gap-1 uppercase tracking-wide">
        {icon}
        {label}
      </div>
      <div className={`text-base font-semibold mt-0.5 ${toneClass}`}>{value}</div>
      {hint && <div className="text-[10px] text-muted-foreground/80">{hint}</div>}
    </div>
  )
}

function FilterSegment({
  options,
  value,
  onChange,
}: {
  options: { value: string; label: string }[]
  value: string
  onChange: (v: string) => void
}) {
  return (
    <div className="inline-flex items-center gap-0.5 border rounded-md p-0.5 bg-muted/30">
      {options.map(({ value: v, label }) => (
        <button
          key={v}
          onClick={() => onChange(v)}
          className={`h-7 px-2.5 text-xs rounded transition ${
            value === v
              ? 'bg-background shadow-sm text-foreground font-medium'
              : 'text-muted-foreground hover:text-foreground'
          }`}
        >
          {label}
        </button>
      ))}
    </div>
  )
}

function LogRow({
  entry,
  expanded,
  onToggle,
}: {
  entry: LogEntry
  expanded: boolean
  onToggle: () => void
}) {
  const isModelCall = entry.kind === 'model_call'
  const mc = entry.modelCall
  const lc = levelClasses(entry.level)
  const sInfo = mc ? statusInfo(mc.status) : null
  const isFailed = mc ? mc.status >= 400 || mc.status === 0 : entry.level.toUpperCase() === 'ERROR'
  const leftBar = isModelCall && sInfo ? sInfo.bar : lc.bar

  return (
    <div className={`relative group transition-colors hover:bg-muted/30 ${isFailed ? lc.bg : ''}`}>
      {/* 左侧色条 */}
      <div className={`absolute left-0 top-0 bottom-0 w-0.5 ${leftBar}`} />

      <div className="flex items-center gap-2 px-3 py-1.5 cursor-pointer" onClick={onToggle}>
        <ChevronDown
          className={`h-3 w-3 text-muted-foreground shrink-0 transition-transform ${expanded ? '' : '-rotate-90'}`}
        />

        <span className="font-mono text-[11px] text-muted-foreground shrink-0 w-20">
          {formatTime(entry.timestamp)}
        </span>

        {/* Level / Status 紧凑 chip */}
        {isModelCall && sInfo ? (
          <span className={`shrink-0 inline-flex items-center gap-0.5 ${sInfo.color}`}>
            {sInfo.icon}
            <span className="font-mono text-[11px]">{sInfo.label}</span>
          </span>
        ) : (
          <span className={`shrink-0 inline-flex items-center text-[10px] font-bold uppercase tracking-wide w-12 ${lc.text}`}>
            {entry.level}
          </span>
        )}

        {/* Source */}
        <span className="shrink-0 text-[11px] text-muted-foreground/80 font-mono truncate max-w-[150px]" title={isModelCall ? `${mc?.endpoint} · ${mc?.apiType}` : entry.target}>
          {isModelCall ? (
            <>
              {mc?.model || mc?.endpoint || '调用'}
            </>
          ) : (
            entry.target.replace(/^kiro_rs::/, '')
          )}
        </span>

        {/* Message */}
        <span className="flex-1 text-xs truncate">
          {isModelCall ? (
            <>
              <span className="font-medium">#{mc?.credentialId}</span>
              <span className="text-muted-foreground"> · {mc?.durationMs}ms</span>
              {mc && mc.retryAttempt > 0 && (
                <span className="ml-1.5 text-amber-600 font-medium">↻ 重试 {mc.retryAttempt}</span>
              )}
              {mc && mc.isStream && (
                <span className="ml-1.5 text-blue-500 text-[10px]">[stream]</span>
              )}
              {mc?.errorSummary && (
                <span className="ml-1.5 text-red-500/80 truncate">— {mc.errorSummary.slice(0, 80)}</span>
              )}
            </>
          ) : (
            entry.message
          )}
        </span>

        {/* 右侧标记 */}
        {isModelCall && (
          <Badge variant="outline" className="h-4 px-1 text-[10px] shrink-0 border-primary/30 text-primary bg-primary/5">
            📡
          </Badge>
        )}
      </div>

      {/* 展开详情 */}
      {expanded && (
        <div className="ml-7 mr-3 mb-2 mt-0.5 p-3 rounded-md bg-muted/40 border border-border/50 text-xs">
          <ExpandedDetails entry={entry} />
        </div>
      )}
    </div>
  )
}

function ExpandedDetails({ entry }: { entry: LogEntry }) {
  const mc = entry.modelCall
  const rows: { label: string; value: React.ReactNode; mono?: boolean; copy?: string }[] = []

  rows.push({
    label: '时间',
    value: new Date(entry.timestamp).toISOString(),
    mono: true,
  })
  rows.push({
    label: '来源',
    value: entry.target,
    mono: true,
  })
  rows.push({
    label: '消息',
    value: entry.message || <span className="text-muted-foreground/60">—</span>,
    copy: entry.message,
  })

  if (mc) {
    rows.push({ label: '凭据', value: `#${mc.credentialId}`, mono: true })
    if (mc.model) rows.push({ label: '模型', value: mc.model, mono: true })
    rows.push({
      label: '端点',
      value: `${mc.endpoint} · ${mc.apiType}${mc.isStream ? ' [stream]' : ''}`,
      mono: true,
    })
    rows.push({ label: '状态', value: String(mc.status), mono: true })
    rows.push({ label: '耗时', value: `${mc.durationMs} ms`, mono: true })
    if (mc.retryAttempt > 0) rows.push({ label: '重试次数', value: String(mc.retryAttempt), mono: true })
    if (mc.errorSummary) {
      rows.push({
        label: '错误摘要',
        value: <span className="text-red-600 dark:text-red-400 break-all">{mc.errorSummary}</span>,
        copy: mc.errorSummary,
      })
    }
  }

  return (
    <div className="space-y-1">
      <div className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
        {rows.map((r, i) => (
          <DetailRow key={i} label={r.label} value={r.value} mono={r.mono} copy={r.copy} />
        ))}
      </div>
      {Object.keys(entry.fields).length > 0 && (
        <>
          <div className="text-[10px] uppercase tracking-wide text-muted-foreground mt-2 mb-1">附加字段</div>
          <div className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 pl-2 border-l-2 border-border/50">
            {Object.entries(entry.fields).map(([k, v]) => (
              <DetailRow key={k} label={k} value={v} mono copy={v} />
            ))}
          </div>
        </>
      )}
    </div>
  )
}

function DetailRow({
  label,
  value,
  mono,
  copy,
}: {
  label: string
  value: React.ReactNode
  mono?: boolean
  copy?: string
}) {
  return (
    <>
      <div className="text-muted-foreground text-[11px] pt-0.5 shrink-0">{label}</div>
      <div className={`${mono ? 'font-mono' : ''} text-[11px] group/row flex items-start gap-1`}>
        <span className="break-all flex-1">{value}</span>
        {copy && (
          <button
            onClick={(e) => {
              e.stopPropagation()
              copyToClipboard(copy)
            }}
            className="opacity-0 group-hover/row:opacity-100 transition shrink-0 h-4 w-4 inline-flex items-center justify-center text-muted-foreground hover:text-foreground"
            title="复制"
          >
            <Copy className="h-3 w-3" />
          </button>
        )}
      </div>
    </>
  )
}
