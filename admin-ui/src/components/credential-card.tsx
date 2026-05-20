import { useState } from 'react'
import { toast } from 'sonner'
import {
  RefreshCw,
  ChevronUp,
  ChevronDown,
  Info,
  Trash2,
  Loader2,
  Pencil,
  Check,
  X,
  RotateCcw,
  Globe,
  Network,
} from 'lucide-react'
import { Card, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Switch } from '@/components/ui/switch'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import type { CredentialStatusItem, BalanceResponse, CachedBalanceInfo } from '@/types/api'
import {
  useSetDisabled,
  useSetPriority,
  useSetEndpoint,
  useSetRegion,
  useResetFailure,
  useDeleteCredential,
  useForceRefreshToken,
} from '@/hooks/use-credentials'

interface CredentialCardProps {
  credential: CredentialStatusItem
  onViewBalance: (id: number) => void
  selected: boolean
  onToggleSelect: () => void
  balance: BalanceResponse | null
  loadingBalance: boolean
  cachedBalance?: CachedBalanceInfo
}

function formatCacheAge(cachedAt: number): string {
  const diff = Date.now() - cachedAt
  if (diff < 0) return '刚刚'
  const seconds = Math.floor(diff / 1000)
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h`
  return `${Math.floor(hours / 24)}d`
}

function formatLastUsed(lastUsedAt: string | null): string {
  if (!lastUsedAt) return '从未使用'
  const date = new Date(lastUsedAt)
  const now = new Date()
  const diff = now.getTime() - date.getTime()
  if (diff < 0) return '刚刚'
  const seconds = Math.floor(diff / 1000)
  if (seconds < 60) return `${seconds} 秒前`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes} 分钟前`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours} 小时前`
  return `${Math.floor(hours / 24)} 天前`
}

function authMethodLabel(method: string | null): string {
  if (method === 'api_key') return 'API Key'
  if (method === 'idc') return 'IdC'
  if (method === 'social') return 'Social'
  return method || ''
}

export function CredentialCard({
  credential,
  onViewBalance,
  selected,
  onToggleSelect,
  balance,
  loadingBalance,
  cachedBalance,
}: CredentialCardProps) {
  const [editingPriority, setEditingPriority] = useState(false)
  const [priorityValue, setPriorityValue] = useState(String(credential.priority))
  const [editingEndpoint, setEditingEndpoint] = useState(false)
  const [endpointValue, setEndpointValue] = useState(credential.endpoint ?? '')
  const [editingRegion, setEditingRegion] = useState(false)
  const [regionValue, setRegionValue] = useState(credential.region ?? '')
  const [apiRegionValue, setApiRegionValue] = useState(credential.apiRegion ?? '')
  const [showDeleteDialog, setShowDeleteDialog] = useState(false)

  const setDisabled = useSetDisabled()
  const setPriority = useSetPriority()
  const setEndpoint = useSetEndpoint()
  const setRegion = useSetRegion()
  const resetFailure = useResetFailure()
  const deleteCredential = useDeleteCredential()
  const forceRefresh = useForceRefreshToken()

  const handleToggleDisabled = () => {
    setDisabled.mutate(
      { id: credential.id, disabled: !credential.disabled },
      {
        onSuccess: (res) => toast.success(res.message),
        onError: (err) => toast.error('操作失败: ' + (err as Error).message),
      }
    )
  }

  const handlePriorityChange = () => {
    const newPriority = parseInt(priorityValue, 10)
    if (isNaN(newPriority) || newPriority < 0) {
      toast.error('优先级必须是非负整数')
      return
    }
    setPriority.mutate(
      { id: credential.id, priority: newPriority },
      {
        onSuccess: (res) => {
          toast.success(res.message)
          setEditingPriority(false)
        },
        onError: (err) => toast.error('操作失败: ' + (err as Error).message),
      }
    )
  }

  const handlePriorityStep = (delta: number) => {
    const newPriority = Math.max(0, credential.priority + delta)
    if (newPriority === credential.priority) return
    setPriority.mutate(
      { id: credential.id, priority: newPriority },
      {
        onSuccess: (res) => toast.success(res.message),
        onError: (err) => toast.error('操作失败: ' + (err as Error).message),
      }
    )
  }

  const handleEndpointChange = () => {
    setEndpoint.mutate(
      { id: credential.id, endpoint: endpointValue || null },
      {
        onSuccess: (res) => {
          toast.success(res.message)
          setEditingEndpoint(false)
        },
        onError: (err) => toast.error('操作失败: ' + (err as Error).message),
      }
    )
  }

  const handleRegionChange = () => {
    setRegion.mutate(
      {
        id: credential.id,
        region: regionValue.trim() || null,
        apiRegion: apiRegionValue.trim() || null,
      },
      {
        onSuccess: (res) => {
          toast.success(res.message)
          setEditingRegion(false)
        },
        onError: (err) => toast.error('操作失败: ' + (err as Error).message),
      }
    )
  }

  const handleReset = () => {
    resetFailure.mutate(credential.id, {
      onSuccess: (res) => toast.success(res.message),
      onError: (err) => toast.error('操作失败: ' + (err as Error).message),
    })
  }

  const handleForceRefresh = () => {
    forceRefresh.mutate(credential.id, {
      onSuccess: (res) => toast.success(res.message),
      onError: (err) => toast.error('刷新失败: ' + (err as Error).message),
    })
  }

  const handleDelete = () => {
    if (!credential.disabled) {
      toast.error('请先禁用凭据再删除')
      setShowDeleteDialog(false)
      return
    }
    deleteCredential.mutate(credential.id, {
      onSuccess: (res) => {
        toast.success(res.message)
        setShowDeleteDialog(false)
      },
      onError: (err) => toast.error('删除失败: ' + (err as Error).message),
    })
  }

  const displayName = credential.email || `凭据 #${credential.id}`
  const subTitle = balance?.subscriptionTitle ?? cachedBalance?.subscriptionTitle ?? null
  const hasFailures = credential.failureCount > 0 || credential.refreshFailureCount > 0

  const effectiveBalance = balance
    ? {
        remaining: balance.remaining,
        limit: balance.usageLimit,
        pct: balance.usagePercentage,
        cached: false,
        age: 0,
        isOverage: Boolean(balance.isOverage),
      }
    : cachedBalance && cachedBalance.usageLimit > 0
      ? {
          remaining: cachedBalance.remaining,
          limit: cachedBalance.usageLimit,
          pct: cachedBalance.usagePercentage,
          cached: true,
          age: cachedBalance.cachedAt,
          isOverage: Boolean(cachedBalance.isOverage),
        }
      : null

  // 阶段 7.12：剩余百分比（不再 clamp，可负数表示超额比例）
  const usagePctRemaining = effectiveBalance ? 100 - effectiveBalance.pct : null
  // 阶梯色 + 超额：紫 / 红 / 黄 / 绿
  const balanceTone: 'overage' | 'red' | 'amber' | 'green' | null = (() => {
    if (!effectiveBalance) return null
    if (effectiveBalance.isOverage || usagePctRemaining! < 0) return 'overage'
    if (usagePctRemaining! < 20) return 'red'
    if (usagePctRemaining! < 50) return 'amber'
    return 'green'
  })()
  const balanceBarColor =
    balanceTone === 'overage'
      ? 'bg-purple-500'
      : balanceTone === 'red'
        ? 'bg-red-500'
        : balanceTone === 'amber'
          ? 'bg-amber-500'
          : 'bg-green-500'
  const balanceTextColor =
    balanceTone === 'overage'
      ? 'text-purple-600 dark:text-purple-400'
      : balanceTone === 'red'
        ? 'text-red-600 dark:text-red-400'
        : balanceTone === 'amber'
          ? 'text-amber-600 dark:text-amber-400'
          : 'text-foreground'
  // 进度条宽度：超额时显示满格 + 视觉提示；正常时按使用百分比
  const balanceBarWidth = effectiveBalance
    ? Math.min(100, Math.max(2, effectiveBalance.pct))
    : 0
  const overageAmount = effectiveBalance && effectiveBalance.isOverage
    ? Math.max(0, -effectiveBalance.remaining)
    : 0

  return (
    <>
      <Card
        className={`group transition-all hover:shadow-md hover:border-primary/30 ${
          credential.isCurrent ? 'ring-1 ring-primary/60 bg-primary/[0.02]' : ''
        } ${credential.disabled ? 'opacity-70' : ''}`}
      >
        <CardContent className="p-3 space-y-2.5">
          {/* 头部：复选框 + 标题 + 启用开关 */}
          <div className="flex items-start gap-2">
            <Checkbox
              checked={selected}
              onCheckedChange={onToggleSelect}
              className="mt-0.5"
            />
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-1.5 mb-1">
                <span className="text-sm font-semibold truncate" title={displayName}>
                  {displayName}
                </span>
                {credential.isCurrent && (
                  <Badge variant="success" className="h-4 px-1 text-[10px] shrink-0">活跃</Badge>
                )}
              </div>
              {/* 徽章组：authMethod / subscription / ARN / disabled */}
              <div className="flex items-center gap-1 flex-wrap">
                {credential.authMethod && (
                  <Badge variant="secondary" className="h-4 px-1.5 text-[10px]">
                    {authMethodLabel(credential.authMethod)}
                  </Badge>
                )}
                {subTitle && (
                  <Badge variant="outline" className="h-4 px-1.5 text-[10px] border-primary/30 text-primary bg-primary/5">
                    {subTitle}
                  </Badge>
                )}
                {credential.hasProfileArn && (
                  <Badge variant="secondary" className="h-4 px-1.5 text-[10px]" title="包含 Profile ARN">ARN</Badge>
                )}
                {/* 阶段 7.12：超额徽章 */}
                {effectiveBalance?.isOverage && !credential.disabled && (
                  <Badge
                    variant="outline"
                    className="h-4 px-1.5 text-[10px] border-purple-500/40 text-purple-600 bg-purple-500/10"
                    title={`已超额 $${overageAmount.toFixed(2)}（订阅范围内凭据优先调用）`}
                  >
                    超额中
                  </Badge>
                )}
                {credential.disabled && (
                  <Badge variant="destructive" className="h-4 px-1.5 text-[10px]">已禁用</Badge>
                )}
                {credential.disabled && credential.disabledReason && (
                  <Badge variant="outline" className="h-4 px-1.5 text-[10px] text-muted-foreground" title={credential.disabledReason}>
                    {credential.disabledReason}
                  </Badge>
                )}
              </div>
            </div>
            <Switch
              checked={!credential.disabled}
              onCheckedChange={handleToggleDisabled}
              disabled={setDisabled.isPending}
              className="shrink-0"
            />
          </div>

          {/* 关键统计行：优先级 / 失败 / 成功 */}
          <div className="flex items-center gap-3 text-xs">
            <StatPill label="优先级">
              {editingPriority ? (
                <span className="inline-flex items-center gap-0.5">
                  <Input
                    type="number"
                    value={priorityValue}
                    onChange={(e) => setPriorityValue(e.target.value)}
                    className="w-12 h-5 text-xs px-1"
                    min="0"
                  />
                  <button
                    onClick={handlePriorityChange}
                    disabled={setPriority.isPending}
                    className="h-5 w-5 inline-flex items-center justify-center text-green-600 hover:bg-green-500/10 rounded"
                  >
                    <Check className="h-3 w-3" />
                  </button>
                  <button
                    onClick={() => {
                      setEditingPriority(false)
                      setPriorityValue(String(credential.priority))
                    }}
                    className="h-5 w-5 inline-flex items-center justify-center text-muted-foreground hover:bg-muted rounded"
                  >
                    <X className="h-3 w-3" />
                  </button>
                </span>
              ) : (
                <button
                  onClick={() => setEditingPriority(true)}
                  className="inline-flex items-center gap-0.5 text-foreground font-semibold hover:text-primary"
                  title="点击编辑"
                >
                  {credential.priority}
                  <Pencil className="h-2.5 w-2.5 opacity-0 group-hover:opacity-50" />
                </button>
              )}
            </StatPill>
            <StatPill label="失败" tone={hasFailures ? 'danger' : 'default'}>
              {credential.failureCount}
              {credential.refreshFailureCount > 0 && (
                <span className="text-red-500/80 ml-0.5">+{credential.refreshFailureCount}</span>
              )}
            </StatPill>
            <StatPill label="成功">{credential.successCount}</StatPill>
          </div>

          {/* 最后调用 + 余额 (两行紧凑) */}
          <div className="text-xs space-y-0.5">
            <div className="flex items-center justify-between text-muted-foreground">
              <span>最后调用</span>
              <span className="text-foreground">{formatLastUsed(credential.lastUsedAt)}</span>
            </div>
            <div className="flex items-center justify-between text-muted-foreground">
              <span>剩余余额</span>
              <span>
                {loadingBalance ? (
                  <Loader2 className="inline w-3 h-3 animate-spin" />
                ) : effectiveBalance ? (
                  <>
                    <span className={`font-medium ${balanceTextColor}`}>
                      {effectiveBalance.remaining < 0 ? '-' : ''}${Math.abs(effectiveBalance.remaining).toFixed(2)}
                    </span>
                    <span className="text-muted-foreground/80"> / ${effectiveBalance.limit.toFixed(2)}</span>
                    <span className={`ml-1 ${balanceTextColor}`}>
                      ({effectiveBalance.isOverage
                        ? `超 ${Math.max(0, effectiveBalance.pct - 100).toFixed(0)}%`
                        : `${Math.max(0, usagePctRemaining!).toFixed(0)}% 剩`}
                      {effectiveBalance.cached && <span className="text-muted-foreground"> · {formatCacheAge(effectiveBalance.age)}前</span>})
                    </span>
                  </>
                ) : (
                  <span className="text-muted-foreground">—</span>
                )}
              </span>
            </div>
          </div>

          {/* 阶梯色用量进度条（绿/黄/红/紫 4 档）*/}
          {effectiveBalance && (
            <div className={`h-1 rounded-full overflow-hidden ${
              balanceTone === 'overage' ? 'bg-purple-500/20' : 'bg-muted'
            }`}>
              <div
                className={`h-full transition-all ${balanceBarColor} ${
                  balanceTone === 'overage' ? 'animate-pulse' : ''
                }`}
                style={{ width: `${balanceBarWidth}%` }}
              />
            </div>
          )}

          {/* Endpoint + Region 编辑（折叠紧凑展示） */}
          <div className="text-xs space-y-1 rounded-md bg-muted/30 px-2 py-1.5">
            {/* Endpoint */}
            <div className="flex items-center gap-1.5">
              <Network className="h-3 w-3 text-muted-foreground shrink-0" />
              <span className="text-muted-foreground shrink-0">Endpoint</span>
              {editingEndpoint ? (
                <div className="inline-flex items-center gap-0.5 ml-auto">
                  <select
                    value={endpointValue}
                    onChange={(e) => setEndpointValue(e.target.value)}
                    className="h-5 px-1 text-xs border rounded bg-background focus:outline-none focus:ring-1 focus:ring-primary/30"
                  >
                    <option value="">默认</option>
                    <option value="ide">ide</option>
                    <option value="cli">cli</option>
                  </select>
                  <button
                    onClick={handleEndpointChange}
                    disabled={setEndpoint.isPending}
                    className="h-5 w-5 inline-flex items-center justify-center text-green-600 hover:bg-green-500/10 rounded"
                  >
                    <Check className="h-3 w-3" />
                  </button>
                  <button
                    onClick={() => {
                      setEditingEndpoint(false)
                      setEndpointValue(credential.endpoint ?? '')
                    }}
                    className="h-5 w-5 inline-flex items-center justify-center text-muted-foreground hover:bg-muted rounded"
                  >
                    <X className="h-3 w-3" />
                  </button>
                </div>
              ) : (
                <button
                  onClick={() => {
                    setEndpointValue(credential.endpoint ?? '')
                    setEditingEndpoint(true)
                  }}
                  className="ml-auto inline-flex items-center gap-1 text-foreground font-medium hover:text-primary"
                  title="点击编辑"
                >
                  <span>{credential.endpoint || credential.effectiveEndpoint}</span>
                  {!credential.endpoint && (
                    <span className="text-[10px] text-muted-foreground/80">(默认)</span>
                  )}
                  <Pencil className="h-2.5 w-2.5 opacity-0 group-hover:opacity-50" />
                </button>
              )}
            </div>

            {/* Region */}
            <div className="flex items-center gap-1.5">
              <Globe className="h-3 w-3 text-muted-foreground shrink-0" />
              <span className="text-muted-foreground shrink-0">Region</span>
              {editingRegion ? (
                <div className="inline-flex items-center gap-0.5 ml-auto flex-wrap justify-end">
                  <Input
                    placeholder="auth"
                    value={regionValue}
                    onChange={(e) => setRegionValue(e.target.value)}
                    className="w-20 h-5 text-xs px-1"
                  />
                  <Input
                    placeholder="api"
                    value={apiRegionValue}
                    onChange={(e) => setApiRegionValue(e.target.value)}
                    className="w-20 h-5 text-xs px-1"
                  />
                  <button
                    onClick={handleRegionChange}
                    disabled={setRegion.isPending}
                    className="h-5 w-5 inline-flex items-center justify-center text-green-600 hover:bg-green-500/10 rounded"
                  >
                    <Check className="h-3 w-3" />
                  </button>
                  <button
                    onClick={() => {
                      setEditingRegion(false)
                      setRegionValue(credential.region ?? '')
                      setApiRegionValue(credential.apiRegion ?? '')
                    }}
                    className="h-5 w-5 inline-flex items-center justify-center text-muted-foreground hover:bg-muted rounded"
                  >
                    <X className="h-3 w-3" />
                  </button>
                </div>
              ) : (
                <button
                  onClick={() => {
                    setRegionValue(credential.region ?? '')
                    setApiRegionValue(credential.apiRegion ?? '')
                    setEditingRegion(true)
                  }}
                  className="ml-auto inline-flex items-center gap-1 text-foreground font-medium hover:text-primary truncate max-w-[60%]"
                  title="点击编辑"
                >
                  <span className="truncate">
                    {credential.region || '全局默认'}
                    {credential.apiRegion && <span className="text-muted-foreground"> · API {credential.apiRegion}</span>}
                  </span>
                  <Pencil className="h-2.5 w-2.5 opacity-0 group-hover:opacity-50 shrink-0" />
                </button>
              )}
            </div>
          </div>

          {/* API Key 显示 */}
          {credential.maskedApiKey && (
            <div className="text-xs text-muted-foreground">
              Key <span className="font-mono text-foreground">{credential.maskedApiKey}</span>
            </div>
          )}

          {/* 代理显示 */}
          {credential.hasProxy && credential.proxyUrl && (
            <div className="text-xs text-muted-foreground truncate" title={credential.proxyUrl}>
              代理 <span className="text-foreground">{credential.proxyUrl}</span>
            </div>
          )}

          {/* 操作按钮 */}
          <div className="flex items-center gap-0.5 pt-1.5 border-t -mx-3 px-3 -mb-0.5">
            <IconButton
              icon={<RotateCcw className="h-3.5 w-3.5" />}
              tooltip="重置失败计数"
              onClick={handleReset}
              disabled={resetFailure.isPending || !hasFailures}
            />
            <IconButton
              icon={<RefreshCw className={`h-3.5 w-3.5 ${forceRefresh.isPending ? 'animate-spin' : ''}`} />}
              tooltip={credential.authMethod === 'api_key' ? 'API Key 凭据无需刷新' : credential.disabled ? '已禁用' : '强制刷新 Token'}
              onClick={handleForceRefresh}
              disabled={forceRefresh.isPending || credential.disabled || credential.authMethod === 'api_key'}
            />
            <IconButton
              icon={<ChevronUp className="h-3.5 w-3.5" />}
              tooltip="提高优先级"
              onClick={() => handlePriorityStep(-1)}
              disabled={setPriority.isPending || credential.priority === 0}
            />
            <IconButton
              icon={<ChevronDown className="h-3.5 w-3.5" />}
              tooltip="降低优先级"
              onClick={() => handlePriorityStep(1)}
              disabled={setPriority.isPending}
            />
            <div className="flex-1" />
            <IconButton
              icon={<Info className="h-3.5 w-3.5" />}
              tooltip="详情"
              variant="primary"
              onClick={() => onViewBalance(credential.id)}
            />
            <IconButton
              icon={<Trash2 className="h-3.5 w-3.5" />}
              tooltip={!credential.disabled ? '需先禁用' : '删除凭据'}
              variant="danger"
              onClick={() => setShowDeleteDialog(true)}
              disabled={!credential.disabled}
            />
          </div>
        </CardContent>
      </Card>

      {/* 删除确认对话框 */}
      <Dialog open={showDeleteDialog} onOpenChange={setShowDeleteDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>确认删除凭据</DialogTitle>
            <DialogDescription>
              您确定要删除凭据 #{credential.id} 吗？此操作无法撤销。
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setShowDeleteDialog(false)}
              disabled={deleteCredential.isPending}
            >
              取消
            </Button>
            <Button
              variant="destructive"
              onClick={handleDelete}
              disabled={deleteCredential.isPending || !credential.disabled}
            >
              确认删除
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}

function StatPill({
  label,
  children,
  tone = 'default',
}: {
  label: string
  children: React.ReactNode
  tone?: 'default' | 'danger'
}) {
  const valueClass = tone === 'danger' ? 'text-red-500 font-medium' : 'text-foreground font-medium'
  return (
    <div className="flex items-baseline gap-1 leading-none">
      <span className="text-[10px] text-muted-foreground uppercase tracking-wide">{label}</span>
      <span className={`text-xs ${valueClass}`}>{children}</span>
    </div>
  )
}

function IconButton({
  icon,
  tooltip,
  onClick,
  disabled,
  variant = 'default',
}: {
  icon: React.ReactNode
  tooltip: string
  onClick: () => void
  disabled?: boolean
  variant?: 'default' | 'primary' | 'danger'
}) {
  const colorClass =
    variant === 'primary'
      ? 'text-primary hover:text-primary hover:bg-primary/10'
      : variant === 'danger'
        ? 'text-red-500 hover:text-red-600 hover:bg-red-500/10'
        : 'text-muted-foreground hover:text-foreground hover:bg-muted'
  return (
    <Button
      size="sm"
      variant="ghost"
      className={`h-7 w-7 p-0 ${colorClass}`}
      title={tooltip}
      onClick={onClick}
      disabled={disabled}
    >
      {icon}
    </Button>
  )
}
