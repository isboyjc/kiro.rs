import { useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'
import { AlertCircle, CheckCircle2, FileJson, Loader2, RefreshCw, Save } from 'lucide-react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { storage } from '@/lib/storage'
import { extractErrorMessage } from '@/lib/utils'
import { useConfig, useConfigRaw, useConfigSchema, useUpdateConfig, useValidateConfig } from '@/hooks/use-config'
import type { ConfigFieldError, ConfigJson, ConfigUpdateResponse } from '@/types/api'
import { ConfigForm } from '@/components/config-form'

interface SettingsDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

type TabKey = 'raw' | 'form'

interface ValidationState {
  valid: boolean | null // null = 未校验，true/false = 已校验
  errors: ConfigFieldError[]
  needsRestart: string[]
  hotReload: string[]
  parseError?: string
}

const EMPTY_VALIDATION: ValidationState = {
  valid: null,
  errors: [],
  needsRestart: [],
  hotReload: [],
}

export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  const [tab, setTab] = useState<TabKey>('form')
  const [text, setText] = useState('')
  const [originalText, setOriginalText] = useState('')
  const [validation, setValidation] = useState<ValidationState>(EMPTY_VALIDATION)
  const [showAdminConfirm, setShowAdminConfirm] = useState(false)
  const [pendingPayload, setPendingPayload] = useState<ConfigJson | null>(null)
  /** 阶段 7.8：可视化表单字段级校验结果（path → 错误消息） */
  const [formErrors, setFormErrors] = useState<Record<string, string>>({})

  // 结构化 config（serde 已补全所有默认值）作为编辑数据源，避免最小化
  // config.json 在表单里显示空白字段
  const { data: configData, isLoading: isLoadingConfig, refetch: refetchConfig } = useConfig(open)
  // raw 仅用于展示文件路径
  const { data: rawData, refetch: refetchRaw } = useConfigRaw(open)
  const { data: schemaData, isLoading: isLoadingSchema } = useConfigSchema(open)
  const { mutateAsync: validateMutate, isPending: isValidating } = useValidateConfig()
  const { mutateAsync: updateMutate, isPending: isUpdating } = useUpdateConfig()
  const isLoadingRaw = isLoadingConfig

  // 打开时把"补全默认值后的 config"序列化为编辑文本（Raw / 表单共用）
  useEffect(() => {
    if (open && configData) {
      const filled = JSON.stringify(configData, null, 2)
      setText(filled)
      setOriginalText(filled)
      setValidation(EMPTY_VALIDATION)
    }
  }, [open, configData])

  // 关闭时重置
  useEffect(() => {
    if (!open) {
      setShowAdminConfirm(false)
      setPendingPayload(null)
    }
  }, [open])

  const hasChanges = text !== originalText
  const parsed = useMemo<ConfigJson | null>(() => {
    if (!text.trim()) return null
    try {
      return JSON.parse(text) as ConfigJson
    } catch {
      return null
    }
  }, [text])
  const parseError = useMemo(() => {
    if (!text.trim()) return 'JSON 不能为空'
    try {
      JSON.parse(text)
      return undefined
    } catch (e) {
      return (e as Error).message
    }
  }, [text])

  const oldAdminKey = useMemo<string | undefined>(() => {
    try {
      const parsed = JSON.parse(originalText) as Record<string, unknown>
      return typeof parsed.adminApiKey === 'string' ? parsed.adminApiKey : undefined
    } catch {
      return undefined
    }
  }, [originalText])

  const handleValidate = async () => {
    if (!parsed) {
      setValidation({ ...EMPTY_VALIDATION, parseError })
      return
    }
    try {
      const res = await validateMutate(parsed)
      setValidation({
        valid: res.valid,
        errors: res.errors,
        needsRestart: res.needsRestart,
        hotReload: res.hotReload,
      })
    } catch (e) {
      toast.error('校验请求失败: ' + extractErrorMessage(e))
    }
  }

  const applySave = async (payload: ConfigJson) => {
    try {
      const res = await updateMutate(payload)
      onSaveSuccess(res)
    } catch (e) {
      toast.error('保存失败: ' + extractErrorMessage(e))
    }
  }

  const onSaveSuccess = (res: ConfigUpdateResponse) => {
    // 1) adminApiKey 热轮换：写入 localStorage，下次请求自动用新 key
    if (res.newAdminApiKey) {
      storage.setApiKey(res.newAdminApiKey)
      toast.success('adminApiKey 已轮换，已自动重连')
    }
    if (res.newApiKey) {
      toast.info('apiKey 已修改，请同步更新 Anthropic 客户端配置')
    }

    // 2) 提示分类
    const restartCount = res.needsRestart.length
    const hotCount = res.hotReload.length
    if (restartCount === 0 && hotCount === 0) {
      toast.success('配置已保存（未检测到字段变化）')
    } else if (restartCount === 0) {
      toast.success(`配置已保存，${hotCount} 个字段立即生效`)
    } else {
      toast.warning(
        `配置已保存。${hotCount} 个字段立即生效；${restartCount} 个字段需重启：${res.needsRestart.join(', ')}`,
        { duration: 8000 }
      )
    }

    // 3) 刷新本地缓存
    refetchConfig()
    refetchRaw()
    setShowAdminConfirm(false)
    setPendingPayload(null)
  }

  const handleSave = async () => {
    if (!parsed) {
      toast.error('JSON 解析失败：' + (parseError ?? '未知错误'))
      return
    }
    // 检查 adminApiKey 是否变化 → 弹二次确认
    const newAdmin = typeof parsed.adminApiKey === 'string' ? parsed.adminApiKey : undefined
    if (newAdmin !== undefined && newAdmin !== oldAdminKey && newAdmin.trim() !== '') {
      setPendingPayload(parsed)
      setShowAdminConfirm(true)
      return
    }
    await applySave(parsed)
  }

  const handleConfirmAdminChange = async () => {
    if (pendingPayload) {
      await applySave(pendingPayload)
    }
  }

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="max-w-4xl">
          <DialogHeader className="pb-2">
            <DialogTitle className="flex items-center gap-2 text-base">
              <div className="h-7 w-7 rounded-md bg-primary/10 flex items-center justify-center">
                <FileJson className="h-4 w-4 text-primary" />
              </div>
              系统配置
            </DialogTitle>
            <DialogDescription className="text-xs flex items-center gap-2 flex-wrap mt-1">
              <span>修改后生效方式</span>
              <span className="inline-flex items-center gap-1">
                <span className="h-1.5 w-1.5 rounded-full bg-green-500" />
                <span>热生效（立即应用）</span>
              </span>
              <span className="inline-flex items-center gap-1">
                <span className="h-1.5 w-1.5 rounded-full bg-amber-500" />
                <span>需重启（写盘但下次启动生效）</span>
              </span>
              <span className="inline-flex items-center gap-1">
                <span className="h-1.5 w-1.5 rounded-full bg-red-500" />
                <span>敏感字段</span>
              </span>
            </DialogDescription>
          </DialogHeader>

          {/* Tab 切换 */}
          <div className="flex items-center gap-0.5 p-1 rounded-md bg-muted/40 w-fit">
            <button
              className={`px-3 py-1.5 text-xs rounded transition ${
                tab === 'form'
                  ? 'bg-background shadow-sm text-foreground font-medium'
                  : 'text-muted-foreground hover:text-foreground'
              }`}
              onClick={() => setTab('form')}
            >
              可视化表单
            </button>
            <button
              className={`px-3 py-1.5 text-xs rounded transition flex items-center gap-1 ${
                tab === 'raw'
                  ? 'bg-background shadow-sm text-foreground font-medium'
                  : 'text-muted-foreground hover:text-foreground'
              }`}
              onClick={() => setTab('raw')}
            >
              Raw JSON
              {parseError && (
                <span className="h-1.5 w-1.5 rounded-full bg-red-500" title={parseError} />
              )}
            </button>
          </div>

          {/* 可视化表单 */}
          {tab === 'form' && (
            <div className="space-y-3">
              {isLoadingSchema || isLoadingRaw || !parsed ? (
                <div className="flex items-center justify-center py-8">
                  {parseError ? (
                    <div className="text-red-600 text-sm flex items-center gap-2">
                      <AlertCircle className="h-4 w-4" />
                      Raw JSON 当前无法解析，请切到 Raw 标签修复后再使用表单
                    </div>
                  ) : (
                    <Loader2 className="h-6 w-6 animate-spin" />
                  )}
                </div>
              ) : schemaData ? (
                <ConfigForm
                  schema={schemaData}
                  value={parsed}
                  onChange={(next) => {
                    // 用 2 空格缩进序列化回 text，保持与 raw 同步
                    setText(JSON.stringify(next, null, 2))
                    setValidation(EMPTY_VALIDATION)
                  }}
                  onValidation={setFormErrors}
                />
              ) : null}
              {hasChanges && (
                <Badge variant="outline" className="text-amber-600">未保存修改</Badge>
              )}
            </div>
          )}

          {/* Raw JSON 编辑区 */}
          {tab === 'raw' && (
            <div className="space-y-3">
              <div className="flex items-center justify-between text-xs text-muted-foreground">
                <span>
                  路径：<code className="font-mono">{rawData?.path ?? '--'}</code>
                </span>
                {hasChanges && (
                  <Badge variant="outline" className="text-amber-600">未保存修改</Badge>
                )}
              </div>

              {isLoadingRaw ? (
                <div className="flex items-center justify-center py-8">
                  <Loader2 className="h-6 w-6 animate-spin" />
                </div>
              ) : (
                <textarea
                  className="w-full h-[420px] font-mono text-xs p-3 border rounded-md bg-background resize-none focus:outline-none focus:ring-2 focus:ring-primary/40"
                  value={text}
                  onChange={(e) => {
                    setText(e.target.value)
                    setValidation(EMPTY_VALIDATION)
                  }}
                  spellCheck={false}
                  placeholder='{"host":"127.0.0.1",...}'
                />
              )}

              {/* 校验/解析反馈 */}
              {parseError && (
                <div className="flex items-start gap-2 p-2 border border-red-200 bg-red-50 dark:bg-red-950/30 rounded text-xs">
                  <AlertCircle className="h-4 w-4 text-red-500 mt-0.5 shrink-0" />
                  <div>
                    <div className="font-medium text-red-700 dark:text-red-400">JSON 解析失败</div>
                    <div className="text-red-600 dark:text-red-300 mt-0.5 font-mono">{parseError}</div>
                  </div>
                </div>
              )}

              {validation.valid === false && (
                <div className="space-y-1 p-2 border border-red-200 bg-red-50 dark:bg-red-950/30 rounded text-xs">
                  <div className="flex items-center gap-1 font-medium text-red-700 dark:text-red-400">
                    <AlertCircle className="h-3.5 w-3.5" /> 校验失败（{validation.errors.length} 项）
                  </div>
                  {validation.errors.map((err, i) => (
                    <div key={i} className="font-mono text-red-600 dark:text-red-300 ml-5">
                      <span className="font-semibold">{err.path}</span>: {err.message}
                    </div>
                  ))}
                </div>
              )}

              {validation.valid === true && (
                <div className="space-y-2 p-2 border border-green-200 bg-green-50 dark:bg-green-950/30 rounded text-xs">
                  <div className="flex items-center gap-1 font-medium text-green-700 dark:text-green-400">
                    <CheckCircle2 className="h-3.5 w-3.5" /> 校验通过
                  </div>
                  {validation.hotReload.length > 0 && (
                    <div className="text-green-700 dark:text-green-400">
                      <Badge variant="secondary" className="mr-1">🟢 热生效</Badge>
                      {validation.hotReload.join(', ')}
                    </div>
                  )}
                  {validation.needsRestart.length > 0 && (
                    <div className="text-amber-700 dark:text-amber-400">
                      <Badge variant="outline" className="mr-1">⚠ 需重启</Badge>
                      {validation.needsRestart.join(', ')}
                    </div>
                  )}
                  {validation.hotReload.length === 0 && validation.needsRestart.length === 0 && (
                    <div className="text-muted-foreground">未检测到字段变化</div>
                  )}
                </div>
              )}
            </div>
          )}

          <DialogFooter className="gap-2 items-center">
            {/* 阶段 7.8：可视化表单 schema 校验错误汇总 */}
            {tab === 'form' && Object.keys(formErrors).length > 0 && (
              <div className="flex-1 text-xs text-red-600 flex items-center gap-1">
                <AlertCircle className="h-3.5 w-3.5 shrink-0" />
                <span>
                  {Object.keys(formErrors).length} 项字段超出范围，无法保存。
                  突破限制可切换到 Raw JSON 编辑
                </span>
              </div>
            )}
            <Button variant="outline" onClick={() => onOpenChange(false)}>取消</Button>
            <Button
              variant="outline"
              onClick={handleValidate}
              disabled={isValidating || !parsed}
            >
              {isValidating ? <Loader2 className="h-4 w-4 mr-1 animate-spin" /> : <RefreshCw className="h-4 w-4 mr-1" />}
              校验
            </Button>
            <Button
              onClick={handleSave}
              disabled={
                isUpdating ||
                !parsed ||
                !hasChanges ||
                (tab === 'form' && Object.keys(formErrors).length > 0)
              }
            >
              {isUpdating ? <Loader2 className="h-4 w-4 mr-1 animate-spin" /> : <Save className="h-4 w-4 mr-1" />}
              保存并应用
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* adminApiKey 二次确认对话框 */}
      <Dialog open={showAdminConfirm} onOpenChange={setShowAdminConfirm}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-amber-600">
              <AlertCircle className="h-5 w-5" />
              确认修改 Admin API Key
            </DialogTitle>
            <DialogDescription>
              你正在修改 adminApiKey。新值生效后：
              <ul className="list-disc ml-5 mt-2 space-y-1">
                <li>当前面板会自动用新 key 重新认证（你无需重新登录）</li>
                <li>其他正在使用旧 key 的工具会立即失效</li>
                <li>新 key 会同时写入 config.json 持久化</li>
              </ul>
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setShowAdminConfirm(false)} disabled={isUpdating}>
              取消
            </Button>
            <Button variant="destructive" onClick={handleConfirmAdminChange} disabled={isUpdating}>
              {isUpdating && <Loader2 className="h-4 w-4 mr-1 animate-spin" />}
              确认轮换
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
