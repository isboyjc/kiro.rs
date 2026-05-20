import { useState, useEffect } from 'react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Loader2, AlertCircle, Boxes, CheckCircle2, XCircle, Play } from 'lucide-react'
import { useCredentialBalance, useCredentialModels } from '@/hooks/use-credentials'
import { testCredentialModel } from '@/api/credentials'
import { parseError } from '@/lib/utils'
import type { CredentialStatusItem } from '@/types/api'

type TestState =
  | { status: 'loading' }
  | { status: 'ok'; reply: string; durationMs: number }
  | { status: 'error'; message: string }

interface CredentialDetailDialogProps {
  credential: CredentialStatusItem | null
  open: boolean
  onOpenChange: (open: boolean) => void
}

function authMethodLabel(method: string | null | undefined): string {
  if (method === 'api_key') return 'API Key'
  if (method === 'idc') return 'IdC'
  if (method === 'social') return 'Social'
  return method || '未知'
}

function formatDate(ts: number | null | undefined): string {
  if (!ts) return '未知'
  return new Date(ts * 1000).toLocaleString('zh-CN')
}

function formatLastUsed(s: string | null): string {
  if (!s) return '从未使用'
  return new Date(s).toLocaleString('zh-CN')
}

function num(n: number): string {
  return n.toLocaleString('zh-CN', { minimumFractionDigits: 2, maximumFractionDigits: 2 })
}

/** 详情行 */
function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-start gap-2 text-xs py-1">
      <span className="text-muted-foreground w-24 shrink-0">{label}</span>
      <span className="flex-1 break-all">{children}</span>
    </div>
  )
}

export function CredentialDetailDialog({ credential, open, onOpenChange }: CredentialDetailDialogProps) {
  const id = credential?.id ?? null
  const { data: balance, isLoading: balLoading, error: balError } = useCredentialBalance(open ? id : null)
  const { data: modelsResp, isLoading: modelsLoading, error: modelsError } = useCredentialModels(open ? id : null)
  const [tests, setTests] = useState<Record<string, TestState>>({})

  // 切换凭据时清空测试结果，避免串号展示
  useEffect(() => {
    setTests({})
  }, [id])

  const handleTest = async (modelId: string) => {
    if (id === null) return
    setTests((prev) => ({ ...prev, [modelId]: { status: 'loading' } }))
    try {
      const result = await testCredentialModel(id, modelId)
      setTests((prev) => ({
        ...prev,
        [modelId]: { status: 'ok', reply: result.reply, durationMs: result.durationMs },
      }))
    } catch (e) {
      setTests((prev) => ({
        ...prev,
        [modelId]: { status: 'error', message: parseError(e).title },
      }))
    }
  }

  if (!credential) return null

  const displayName = credential.email || `凭据 #${credential.id}`
  const hasFailures = credential.failureCount > 0 || credential.refreshFailureCount > 0
  const remainingPct = balance ? Math.max(0, 100 - balance.usagePercentage) : null

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader className="pb-2">
          <DialogTitle className="flex items-center gap-2 text-base">
            <span className="truncate">{displayName}</span>
            <span className="text-xs font-normal text-muted-foreground">#{credential.id}</span>
            {credential.isCurrent && <Badge variant="success" className="h-4 px-1 text-[10px]">当前</Badge>}
            {credential.disabled && <Badge variant="destructive" className="h-4 px-1 text-[10px]">已禁用</Badge>}
          </DialogTitle>
        </DialogHeader>

        <div className="max-h-[560px] overflow-y-auto space-y-4 pr-1">
          {/* 余额区 */}
          <section className="rounded-lg border bg-card p-3">
            <div className="text-xs font-medium text-muted-foreground mb-2">余额 / 订阅</div>
            {balLoading ? (
              <div className="flex items-center justify-center py-4">
                <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
              </div>
            ) : balError ? (
              <div className="text-xs text-red-500 flex items-center gap-1">
                <AlertCircle className="h-3.5 w-3.5" /> {parseError(balError).title}
              </div>
            ) : balance ? (
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <span className="text-sm font-semibold">{balance.subscriptionTitle || '未知订阅'}</span>
                  {balance.isOverage && (
                    <Badge variant="outline" className="h-4 px-1.5 text-[10px] border-purple-500/40 text-purple-600 bg-purple-500/10">
                      超额中
                    </Badge>
                  )}
                </div>
                <div className="flex justify-between text-xs">
                  <span>已用 ${num(balance.currentUsage)}</span>
                  <span>限额 ${num(balance.usageLimit)}</span>
                </div>
                <Progress value={Math.min(100, balance.usagePercentage)} />
                <div className="flex justify-between text-xs text-muted-foreground">
                  <span>
                    剩余{' '}
                    <span className={balance.remaining < 0 ? 'text-purple-600 font-medium' : 'text-green-600 font-medium'}>
                      {balance.remaining < 0 ? '-' : ''}${num(Math.abs(balance.remaining))}
                    </span>
                    {remainingPct != null && <> ({remainingPct.toFixed(0)}%)</>}
                  </span>
                  <span>重置 {formatDate(balance.nextResetAt)}</span>
                </div>
              </div>
            ) : (
              <div className="text-xs text-muted-foreground">无余额数据</div>
            )}
          </section>

          {/* 基本信息 */}
          <section className="rounded-lg border bg-card p-3">
            <div className="text-xs font-medium text-muted-foreground mb-1">基本信息</div>
            <div className="grid grid-cols-2 gap-x-4">
              <Row label="认证方式">
                <Badge variant="secondary" className="h-4 px-1.5 text-[10px]">{authMethodLabel(credential.authMethod)}</Badge>
              </Row>
              <Row label="优先级">{credential.priority}</Row>
              <Row label="失败次数">
                <span className={hasFailures ? 'text-red-500 font-medium' : ''}>{credential.failureCount}</span>
              </Row>
              <Row label="刷新失败">
                <span className={credential.refreshFailureCount > 0 ? 'text-red-500 font-medium' : ''}>{credential.refreshFailureCount}</span>
              </Row>
              <Row label="成功次数">{credential.successCount}</Row>
              <Row label="最后调用">{formatLastUsed(credential.lastUsedAt)}</Row>
              <Row label="Token 过期">{credential.expiresAt ? new Date(credential.expiresAt).toLocaleString('zh-CN') : '—'}</Row>
              <Row label="Profile ARN">{credential.hasProfileArn ? '是' : '否'}</Row>
              {credential.disabled && credential.disabledReason && (
                <Row label="禁用原因">{credential.disabledReason}</Row>
              )}
            </div>
          </section>

          {/* 端点 / 区域 / 代理 */}
          <section className="rounded-lg border bg-card p-3">
            <div className="text-xs font-medium text-muted-foreground mb-1">端点 / 区域 / 代理</div>
            <div className="grid grid-cols-2 gap-x-4">
              <Row label="Endpoint">
                {credential.endpoint || credential.effectiveEndpoint}
                {!credential.endpoint && <span className="text-muted-foreground/70"> (默认)</span>}
              </Row>
              <Row label="生效 Endpoint">{credential.effectiveEndpoint}</Row>
              <Row label="Region">{credential.region || '全局默认'}</Row>
              <Row label="API Region">{credential.apiRegion || '—'}</Row>
              <Row label="代理">{credential.hasProxy ? (credential.proxyUrl || '已配置') : '无'}</Row>
              {credential.maskedApiKey && <Row label="API Key">{credential.maskedApiKey}</Row>}
            </div>
            {(credential.refreshTokenHash || credential.apiKeyHash) && (
              <div className="mt-1">
                <Row label="指纹">
                  <span className="font-mono text-[10px] text-muted-foreground">
                    {credential.refreshTokenHash || credential.apiKeyHash}
                  </span>
                </Row>
              </div>
            )}
          </section>

          {/* 可用模型 */}
          <section className="rounded-lg border bg-card p-3">
            <div className="text-xs font-medium text-muted-foreground mb-2 flex items-center gap-1">
              <Boxes className="h-3.5 w-3.5" /> 可用模型
              {modelsResp && <span className="text-[10px]">({modelsResp.models.length})</span>}
            </div>
            {modelsLoading ? (
              <div className="flex items-center justify-center py-4">
                <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
              </div>
            ) : modelsError ? (
              <div className="text-xs text-red-500 flex items-center gap-1">
                <AlertCircle className="h-3.5 w-3.5" /> {parseError(modelsError).title}
              </div>
            ) : modelsResp && modelsResp.models.length > 0 ? (
              <div className="space-y-1.5">
                {modelsResp.models.map((m) => {
                  const t = tests[m.modelId]
                  return (
                    <div key={m.modelId} className="py-1 border-b border-border/40 last:border-0">
                      <div className="flex items-center justify-between gap-2 text-xs">
                        <div className="min-w-0">
                          <div className="font-medium truncate">{m.modelName || m.modelId}</div>
                          <div className="font-mono text-[10px] text-muted-foreground truncate">{m.modelId}</div>
                        </div>
                        <div className="flex items-center gap-2 shrink-0">
                          <div className="text-right text-[10px] text-muted-foreground">
                            {m.rateMultiplier != null && <div>×{m.rateMultiplier} 费率</div>}
                            {m.tokenLimits?.maxInputTokens != null && (
                              <div>{(m.tokenLimits.maxInputTokens / 1000).toFixed(0)}K ctx</div>
                            )}
                          </div>
                          <Button
                            size="sm"
                            variant="outline"
                            className="h-6 px-2 text-[10px]"
                            disabled={t?.status === 'loading'}
                            onClick={() => handleTest(m.modelId)}
                          >
                            {t?.status === 'loading' ? (
                              <Loader2 className="h-3 w-3 animate-spin" />
                            ) : (
                              <Play className="h-3 w-3" />
                            )}
                            <span className="ml-1">测试</span>
                          </Button>
                        </div>
                      </div>
                      {t?.status === 'ok' && (
                        <div className="mt-1 flex items-start gap-1 text-[10px] text-green-600">
                          <CheckCircle2 className="h-3 w-3 mt-px shrink-0" />
                          <span className="break-all">
                            可用（{t.durationMs}ms）：{t.reply}
                          </span>
                        </div>
                      )}
                      {t?.status === 'error' && (
                        <div className="mt-1 flex items-start gap-1 text-[10px] text-red-500">
                          <XCircle className="h-3 w-3 mt-px shrink-0" />
                          <span className="break-all">{t.message}</span>
                        </div>
                      )}
                    </div>
                  )
                })}
              </div>
            ) : (
              <div className="text-xs text-muted-foreground">无可用模型数据</div>
            )}
          </section>
        </div>
      </DialogContent>
    </Dialog>
  )
}
