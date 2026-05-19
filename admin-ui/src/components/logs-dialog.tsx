import { useMemo, useState } from 'react'
import { toast } from 'sonner'
import {
  AlertCircle,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  FileText,
  Loader2,
  Pause,
  Play,
  Radio,
  Trash2,
  XCircle,
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

function levelColor(level: string): string {
  const u = level.toUpperCase()
  if (u === 'ERROR') return 'text-red-500'
  if (u === 'WARN') return 'text-amber-500'
  return 'text-blue-500'
}

function statusBadge(status: number): { color: string; icon: React.ReactNode; label: string } {
  if (status === 0) {
    return { color: 'text-red-500', icon: <XCircle className="h-3 w-3" />, label: 'NET' }
  }
  if (status >= 400) {
    return { color: 'text-red-500', icon: <XCircle className="h-3 w-3" />, label: String(status) }
  }
  return { color: 'text-green-600', icon: <CheckCircle2 className="h-3 w-3" />, label: String(status) }
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

  const toggleExpand = (timestamp: number) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(timestamp)) {
        next.delete(timestamp)
      } else {
        next.add(timestamp)
      }
      return next
    })
  }

  const stats = data?.stats

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-5xl">
        <DialogHeader className="pb-2">
          <DialogTitle className="flex items-center gap-2 text-base">
            <div className="h-7 w-7 rounded-md bg-primary/10 flex items-center justify-center">
              <FileText className="h-4 w-4 text-primary" />
            </div>
            日志查看
            {autoRefresh && (
              <span className="ml-1 inline-flex items-center gap-1 text-xs font-normal text-green-600">
                <Radio className="h-3 w-3 animate-pulse" /> 实时
              </span>
            )}
          </DialogTitle>
          <DialogDescription className="text-xs mt-1">
            内存环形缓冲，重启丢失。已缓存{' '}
            <span className="font-mono">{data?.totalBuffered ?? 0}</span> / {data?.capacity ?? 0} 条
          </DialogDescription>
        </DialogHeader>

        {/* 统计条 (最近 5 分钟 ModelCall) */}
        {stats && stats.total > 0 && (
          <div className="flex items-center gap-3 px-3 py-2 rounded-md bg-muted/30 text-xs">
            <span className="text-muted-foreground">最近 5 分钟模型调用:</span>
            <span>
              共 <span className="font-medium text-foreground">{stats.total}</span> 次
            </span>
            <span className="text-green-600">
              ✓ {stats.success}
            </span>
            <span className="text-red-500">
              ✗ {stats.failed}
            </span>
            <span className="text-muted-foreground">·</span>
            <span>
              成功率{' '}
              <span className="font-medium text-foreground">
                {stats.total > 0 ? ((stats.success / stats.total) * 100).toFixed(1) : '0'}%
              </span>
            </span>
            <span className="text-muted-foreground">·</span>
            <span>
              平均 <span className="font-medium text-foreground">{stats.avgMs}</span>ms
            </span>
            <span className="text-muted-foreground">·</span>
            <span>
              P95 <span className="font-medium text-foreground">{stats.p95Ms}</span>ms
            </span>
          </div>
        )}

        {/* 工具栏 */}
        <div className="space-y-2">
          <div className="flex items-center gap-2 flex-wrap">
            {/* 类型分段 */}
            <div className="flex items-center gap-0.5 border rounded-md p-0.5 bg-muted/30">
              {([
                ['all', '全部'],
                ['generic', '系统'],
                ['model_call', '模型调用'],
              ] as const).map(([k, label]) => (
                <button
                  key={k}
                  onClick={() => setKindFilter(k)}
                  className={`h-7 px-2 text-xs rounded transition ${
                    kindFilter === k
                      ? 'bg-background shadow-sm text-foreground font-medium'
                      : 'text-muted-foreground hover:text-foreground'
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>

            {/* 等级分段 */}
            <div className="flex items-center gap-0.5 border rounded-md p-0.5 bg-muted/30">
              {LEVEL_OPTIONS.map(({ value, label }) => (
                <button
                  key={value}
                  onClick={() => setLevelFilter(value)}
                  className={`h-7 px-2 text-xs rounded transition ${
                    levelFilter === value
                      ? 'bg-background shadow-sm text-foreground font-medium'
                      : 'text-muted-foreground hover:text-foreground'
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>

            <input
              type="text"
              placeholder="🔍 搜索消息 / target / 字段"
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              className="flex-1 min-w-[200px] h-8 px-2 text-xs border rounded-md bg-background focus:outline-none focus:ring-2 focus:ring-primary/30"
            />

            <label className="inline-flex items-center gap-1 text-xs text-muted-foreground cursor-pointer select-none">
              <Switch
                checked={onlyFailed}
                onCheckedChange={setOnlyFailed}
                className="scale-75"
              />
              仅失败
            </label>
          </div>

          <div className="flex items-center gap-2 text-xs">
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
              {isLoading ? <Loader2 className="h-3 w-3 mr-1 animate-spin" /> : null}
              立即刷新
            </Button>
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
        <div className="border rounded-md max-h-[500px] overflow-y-auto bg-background">
          {error ? (
            <div className="p-6 text-center text-sm text-red-500">
              <AlertCircle className="h-5 w-5 mx-auto mb-2" />
              加载失败：{extractErrorMessage(error)}
            </div>
          ) : isLoading && !data ? (
            <div className="p-6 text-center text-sm text-muted-foreground">
              <Loader2 className="h-5 w-5 mx-auto mb-2 animate-spin" />
              加载中...
            </div>
          ) : !data?.entries.length ? (
            <div className="p-6 text-center text-sm text-muted-foreground">
              没有匹配当前过滤条件的日志
            </div>
          ) : (
            <div className="divide-y">
              {data.entries.map((entry) => (
                <LogRow
                  key={`${entry.timestamp}-${entry.target}-${entry.message.slice(0, 20)}`}
                  entry={entry}
                  expanded={expanded.has(entry.timestamp)}
                  onToggle={() => toggleExpand(entry.timestamp)}
                />
              ))}
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
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
  const statusInfo = mc ? statusBadge(mc.status) : null
  const isFailed = mc ? mc.status >= 400 || mc.status === 0 : entry.level.toUpperCase() === 'ERROR'

  return (
    <div
      className={`px-2 py-1.5 hover:bg-muted/30 transition text-xs ${isFailed ? 'bg-red-50/40 dark:bg-red-950/10' : ''}`}
    >
      <div className="flex items-start gap-2 cursor-pointer" onClick={onToggle}>
        <button className="h-4 w-4 inline-flex items-center justify-center text-muted-foreground shrink-0 mt-0.5">
          {expanded ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />}
        </button>
        <span className="font-mono text-muted-foreground shrink-0 mt-0.5">{formatTime(entry.timestamp)}</span>
        <span className={`font-semibold uppercase shrink-0 mt-0.5 w-12 ${levelColor(entry.level)}`}>
          {entry.level}
        </span>
        {isModelCall && statusInfo ? (
          <span className={`shrink-0 inline-flex items-center gap-0.5 ${statusInfo.color}`}>
            {statusInfo.icon}
            <span className="font-mono">{statusInfo.label}</span>
          </span>
        ) : (
          <span className="shrink-0 text-muted-foreground/70 font-mono truncate max-w-[140px]">
            {entry.target.replace(/^kiro_rs::/, '')}
          </span>
        )}
        <span className="flex-1 truncate">
          {entry.message}
          {isModelCall && mc?.retryAttempt ? (
            <span className="ml-2 text-amber-600">↻{mc.retryAttempt}</span>
          ) : null}
        </span>
        {isModelCall && (
          <Badge variant="outline" className="h-4 px-1 text-[10px] shrink-0">
            📡 调用
          </Badge>
        )}
      </div>

      {/* 展开详情 */}
      {expanded && (
        <div className="mt-1.5 ml-6 p-2 rounded bg-muted/40 font-mono text-[11px] space-y-0.5">
          <div>
            <span className="text-muted-foreground">timestamp:</span> {new Date(entry.timestamp).toISOString()}
          </div>
          <div>
            <span className="text-muted-foreground">target:</span> {entry.target}
          </div>
          <div>
            <span className="text-muted-foreground">kind:</span> {entry.kind}
          </div>
          {mc && (
            <>
              <div>
                <span className="text-muted-foreground">credentialId:</span> #{mc.credentialId}
              </div>
              {mc.model && (
                <div>
                  <span className="text-muted-foreground">model:</span> {mc.model}
                </div>
              )}
              <div>
                <span className="text-muted-foreground">endpoint:</span> {mc.endpoint} · {mc.apiType}
                {mc.isStream && <span className="ml-1 text-blue-500">[stream]</span>}
              </div>
              <div>
                <span className="text-muted-foreground">status / duration:</span>{' '}
                {mc.status} / {mc.durationMs}ms
              </div>
              {mc.retryAttempt > 0 && (
                <div>
                  <span className="text-muted-foreground">retryAttempt:</span> {mc.retryAttempt}
                </div>
              )}
              {mc.errorSummary && (
                <div className="text-red-500 break-all">
                  <span className="text-muted-foreground">error:</span> {mc.errorSummary}
                </div>
              )}
            </>
          )}
          {Object.keys(entry.fields).length > 0 && (
            <div>
              <span className="text-muted-foreground">fields:</span>
              {Object.entries(entry.fields).map(([k, v]) => (
                <div key={k} className="ml-3 break-all">
                  <span className="text-muted-foreground">{k}:</span> {v}
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
