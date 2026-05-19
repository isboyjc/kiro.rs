import { useState } from 'react'
import { toast } from 'sonner'
import {
  RefreshCw,
  ChevronUp,
  ChevronDown,
  Wallet,
  Trash2,
  Loader2,
  Pencil,
  Check,
  X,
  RotateCcw,
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
  const [showDeleteDialog, setShowDeleteDialog] = useState(false)

  const setDisabled = useSetDisabled()
  const setPriority = useSetPriority()
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

  // 余额展示数据（live 优先，否则 cached）
  const effectiveBalance = balance
    ? { remaining: balance.remaining, limit: balance.usageLimit, pct: balance.usagePercentage, cached: false, age: 0 }
    : cachedBalance && cachedBalance.usageLimit > 0
      ? { remaining: cachedBalance.remaining, limit: cachedBalance.usageLimit, pct: cachedBalance.usagePercentage, cached: true, age: cachedBalance.cachedAt }
      : null

  return (
    <>
      <Card
        className={`transition-all hover:shadow-md ${
          credential.isCurrent ? 'ring-1 ring-primary/60 bg-primary/[0.02]' : ''
        } ${credential.disabled ? 'opacity-70' : ''}`}
      >
        <CardContent className="p-3 space-y-2">
          {/* 头部：复选框 + 标题 + 启用开关 */}
          <div className="flex items-start gap-2">
            <Checkbox
              checked={selected}
              onCheckedChange={onToggleSelect}
              className="mt-0.5"
            />
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-1.5 flex-wrap">
                <span className="text-sm font-medium truncate" title={displayName}>
                  {displayName}
                </span>
                {credential.isCurrent && (
                  <Badge variant="success" className="h-4 px-1 text-[10px]">当前</Badge>
                )}
                {credential.disabled && (
                  <Badge variant="destructive" className="h-4 px-1 text-[10px]">禁用</Badge>
                )}
                {credential.authMethod && (
                  <Badge variant="secondary" className="h-4 px-1 text-[10px]">
                    {authMethodLabel(credential.authMethod)}
                  </Badge>
                )}
                {credential.endpoint && (
                  <Badge variant="outline" className="h-4 px-1 text-[10px]">{credential.endpoint}</Badge>
                )}
                {credential.hasProfileArn && (
                  <Badge variant="secondary" className="h-4 px-1 text-[10px]" title="包含 Profile ARN">ARN</Badge>
                )}
              </div>
              {credential.disabled && credential.disabledReason && (
                <div className="text-[10px] text-muted-foreground mt-0.5 truncate">
                  原因：{credential.disabledReason}
                </div>
              )}
            </div>
            <Switch
              checked={!credential.disabled}
              onCheckedChange={handleToggleDisabled}
              disabled={setDisabled.isPending}
              className="shrink-0"
            />
          </div>

          {/* 信息区：单列紧凑 */}
          <div className="text-xs space-y-1">
            {/* 优先级 + 失败计数 */}
            <div className="flex items-center gap-3 text-muted-foreground">
              <div className="flex items-center gap-1">
                <span>优先级</span>
                {editingPriority ? (
                  <span className="inline-flex items-center gap-0.5">
                    <Input
                      type="number"
                      value={priorityValue}
                      onChange={(e) => setPriorityValue(e.target.value)}
                      className="w-14 h-6 text-xs px-1"
                      min="0"
                    />
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-6 w-6 p-0"
                      onClick={handlePriorityChange}
                      disabled={setPriority.isPending}
                    >
                      <Check className="h-3 w-3" />
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-6 w-6 p-0"
                      onClick={() => {
                        setEditingPriority(false)
                        setPriorityValue(String(credential.priority))
                      }}
                    >
                      <X className="h-3 w-3" />
                    </Button>
                  </span>
                ) : (
                  <button
                    onClick={() => setEditingPriority(true)}
                    className="inline-flex items-center gap-0.5 text-foreground font-medium hover:text-primary group"
                    title="点击编辑"
                  >
                    {credential.priority}
                    <Pencil className="h-2.5 w-2.5 opacity-0 group-hover:opacity-60" />
                  </button>
                )}
              </div>
              <span className="text-muted-foreground/50">·</span>
              <div>
                失败 <span className={hasFailures ? 'text-red-500 font-medium' : 'text-foreground'}>{credential.failureCount}</span>
                {credential.refreshFailureCount > 0 && (
                  <> · 刷新失败 <span className="text-red-500 font-medium">{credential.refreshFailureCount}</span></>
                )}
              </div>
              <span className="text-muted-foreground/50">·</span>
              <div>成功 <span className="text-foreground">{credential.successCount}</span></div>
            </div>

            {/* 订阅 + 最后调用 */}
            <div className="flex items-center gap-2 text-muted-foreground">
              <span>
                订阅 <span className="text-foreground font-medium">
                  {loadingBalance ? <Loader2 className="inline w-3 h-3 animate-spin" /> : (subTitle || '—')}
                </span>
              </span>
              <span className="text-muted-foreground/50">·</span>
              <span className="truncate">{formatLastUsed(credential.lastUsedAt)}</span>
            </div>

            {/* 余额行 */}
            <div className="text-muted-foreground">
              {loadingBalance ? (
                <span className="text-foreground"><Loader2 className="inline w-3 h-3 animate-spin" /> 余额加载中</span>
              ) : effectiveBalance ? (
                <>
                  余额 <span className="text-foreground font-medium">
                    ${effectiveBalance.remaining.toFixed(2)} / ${effectiveBalance.limit.toFixed(2)}
                  </span>
                  <span className="ml-1">
                    ({(100 - effectiveBalance.pct).toFixed(0)}%
                    {effectiveBalance.cached && <> · {formatCacheAge(effectiveBalance.age)}前缓存</>})
                  </span>
                </>
              ) : (
                <span>余额未知</span>
              )}
            </div>

            {/* API key (仅 api_key 凭据) */}
            {credential.maskedApiKey && (
              <div className="text-muted-foreground">
                Key <span className="font-mono text-foreground">{credential.maskedApiKey}</span>
              </div>
            )}

            {/* 代理 */}
            {credential.hasProxy && credential.proxyUrl && (
              <div className="text-muted-foreground truncate" title={credential.proxyUrl}>
                代理 <span className="text-foreground">{credential.proxyUrl}</span>
              </div>
            )}
          </div>

          {/* 操作按钮：图标 + tooltip */}
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
              icon={<Wallet className="h-3.5 w-3.5" />}
              tooltip="查看余额详情"
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
