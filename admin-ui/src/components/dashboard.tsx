import { useState, useEffect, useMemo, useRef } from 'react'
import { RefreshCw, LogOut, Moon, Sun, Server, Plus, Upload, Trash2, RotateCcw, CheckCircle2, ArrowUp, ArrowDown, Settings, Search, X, FileText } from 'lucide-react'
import { useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { storage } from '@/lib/storage'
import { Card } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { CredentialCard } from '@/components/credential-card'
import { BalanceDialog } from '@/components/balance-dialog'
import { AddCredentialDialog } from '@/components/add-credential-dialog'
import { ImportTokenJsonDialog } from '@/components/import-token-json-dialog'
import { SettingsDialog } from '@/components/settings-dialog'
import { LogsDialog } from '@/components/logs-dialog'
import { BatchVerifyDialog, type VerifyResult } from '@/components/batch-verify-dialog'
import { useCredentials, useDeleteCredential, useResetFailure, useCachedBalances } from '@/hooks/use-credentials'
import { getCredentialBalance, forceRefreshToken } from '@/api/credentials'
import { extractErrorMessage } from '@/lib/utils'
import type { BalanceResponse, CachedBalanceInfo } from '@/types/api'

interface DashboardProps {
  onLogout: () => void
}

type SortField = 'default' | 'id' | 'balance' | 'priority' | 'lastUsed'
type SortOrder = 'asc' | 'desc'
type StatusFilter = 'all' | 'enabled' | 'disabled' | 'failed' | 'current'

export function Dashboard({ onLogout }: DashboardProps) {
  const [selectedCredentialId, setSelectedCredentialId] = useState<number | null>(null)
  const [balanceDialogOpen, setBalanceDialogOpen] = useState(false)
  const [addDialogOpen, setAddDialogOpen] = useState(false)
  const [importTokenJsonDialogOpen, setImportTokenJsonDialogOpen] = useState(false)
  const [settingsDialogOpen, setSettingsDialogOpen] = useState(false)
  const [logsDialogOpen, setLogsDialogOpen] = useState(false)
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set())
  const [verifyDialogOpen, setVerifyDialogOpen] = useState(false)
  const [verifying, setVerifying] = useState(false)
  const [verifyProgress, setVerifyProgress] = useState({ current: 0, total: 0 })
  const [verifyResults, setVerifyResults] = useState<Map<number, VerifyResult>>(new Map())
  const [balanceMap, setBalanceMap] = useState<Map<number, BalanceResponse>>(new Map())
  const [loadingBalanceIds, setLoadingBalanceIds] = useState<Set<number>>(new Set())
  const [queryingInfo, setQueryingInfo] = useState(false)
  const [queryInfoProgress, setQueryInfoProgress] = useState({ current: 0, total: 0 })
  const [batchRefreshing, setBatchRefreshing] = useState(false)
  const [batchRefreshProgress, setBatchRefreshProgress] = useState({ current: 0, total: 0 })
  const cancelVerifyRef = useRef(false)
  const [currentPage, setCurrentPage] = useState(1)
  const itemsPerPage = 12
  const [sortField, setSortField] = useState<SortField>('default')
  const [sortOrder, setSortOrder] = useState<SortOrder>('asc')
  const [searchTerm, setSearchTerm] = useState('')
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all')
  const [darkMode, setDarkMode] = useState(() => {
    if (typeof window !== 'undefined') {
      return document.documentElement.classList.contains('dark')
    }
    return false
  })

  const queryClient = useQueryClient()
  const { data, isLoading, error, refetch } = useCredentials()
  const { mutate: deleteCredential } = useDeleteCredential()
  const { mutate: resetFailure } = useResetFailure()
  const { data: cachedBalancesData } = useCachedBalances()

  // 阶段 5.3d：构建 id -> CachedBalanceInfo 映射，供 card 降级展示 + 排序使用
  const cachedBalanceMap = useMemo(() => {
    const m = new Map<number, CachedBalanceInfo>()
    cachedBalancesData?.balances.forEach(b => m.set(b.id, b))
    return m
  }, [cachedBalancesData])

  // 阶段 7.4：先过滤再排序
  const filteredCredentials = useMemo(() => {
    const credentials = data?.credentials || []
    const term = searchTerm.trim().toLowerCase()
    return credentials.filter(c => {
      // 状态过滤
      if (statusFilter === 'enabled' && c.disabled) return false
      if (statusFilter === 'disabled' && !c.disabled) return false
      if (statusFilter === 'failed' && c.failureCount === 0 && c.refreshFailureCount === 0) return false
      if (statusFilter === 'current' && !c.isCurrent) return false
      // 搜索过滤（id / email / endpoint / authMethod）
      if (term) {
        const haystack = [
          String(c.id),
          c.email ?? '',
          c.endpoint ?? '',
          c.authMethod ?? '',
          c.maskedApiKey ?? '',
        ].join(' ').toLowerCase()
        if (!haystack.includes(term)) return false
      }
      return true
    })
  }, [data?.credentials, searchTerm, statusFilter])

  const sortedCredentials = useMemo(() => {
    if (sortField === 'default') return filteredCredentials
    return [...filteredCredentials].sort((a, b) => {
      let cmp = 0
      if (sortField === 'id') {
        cmp = a.id - b.id
      } else if (sortField === 'priority') {
        cmp = a.priority - b.priority
      } else if (sortField === 'balance') {
        const balA = cachedBalanceMap.get(a.id)?.remaining ?? -Infinity
        const balB = cachedBalanceMap.get(b.id)?.remaining ?? -Infinity
        cmp = balA - balB
      } else if (sortField === 'lastUsed') {
        const ta = a.lastUsedAt ? new Date(a.lastUsedAt).getTime() : 0
        const tb = b.lastUsedAt ? new Date(b.lastUsedAt).getTime() : 0
        cmp = ta - tb
      }
      return sortOrder === 'asc' ? cmp : -cmp
    })
  }, [filteredCredentials, sortField, sortOrder, cachedBalanceMap])

  // 计算分页
  const totalPages = Math.ceil(sortedCredentials.length / itemsPerPage)
  const startIndex = (currentPage - 1) * itemsPerPage
  const endIndex = startIndex + itemsPerPage
  const currentCredentials = sortedCredentials.slice(startIndex, endIndex)
  const hasActiveFilter = statusFilter !== 'all' || searchTerm.trim() !== ''

  const disabledCredentialCount = data?.credentials.filter(credential => credential.disabled).length || 0
  const selectedDisabledCount = Array.from(selectedIds).filter(id => {
    const credential = data?.credentials.find(c => c.id === id)
    return Boolean(credential?.disabled)
  }).length

  // 当凭据列表或过滤条件变化时重置到第一页
  useEffect(() => {
    setCurrentPage(1)
  }, [data?.credentials.length, searchTerm, statusFilter])

  // 只保留当前仍存在的凭据缓存，避免删除后残留旧数据
  useEffect(() => {
    if (!data?.credentials) {
      setBalanceMap(new Map())
      setLoadingBalanceIds(new Set())
      return
    }

    const validIds = new Set(data.credentials.map(credential => credential.id))

    setBalanceMap(prev => {
      const next = new Map<number, BalanceResponse>()
      prev.forEach((value, id) => {
        if (validIds.has(id)) {
          next.set(id, value)
        }
      })
      return next.size === prev.size ? prev : next
    })

    setLoadingBalanceIds(prev => {
      if (prev.size === 0) {
        return prev
      }
      const next = new Set<number>()
      prev.forEach(id => {
        if (validIds.has(id)) {
          next.add(id)
        }
      })
      return next.size === prev.size ? prev : next
    })
  }, [data?.credentials])

  const toggleDarkMode = () => {
    setDarkMode(!darkMode)
    document.documentElement.classList.toggle('dark')
  }

  const handleViewBalance = (id: number) => {
    setSelectedCredentialId(id)
    setBalanceDialogOpen(true)
  }

  const handleRefresh = () => {
    refetch()
    toast.success('已刷新凭据列表')
  }

  const handleLogout = () => {
    storage.removeApiKey()
    queryClient.clear()
    onLogout()
  }

  // 选择管理
  const toggleSelect = (id: number) => {
    const newSelected = new Set(selectedIds)
    if (newSelected.has(id)) {
      newSelected.delete(id)
    } else {
      newSelected.add(id)
    }
    setSelectedIds(newSelected)
  }

  const deselectAll = () => {
    setSelectedIds(new Set())
  }

  // 批量删除（仅删除已禁用项）
  const handleBatchDelete = async () => {
    if (selectedIds.size === 0) {
      toast.error('请先选择要删除的凭据')
      return
    }

    const disabledIds = Array.from(selectedIds).filter(id => {
      const credential = data?.credentials.find(c => c.id === id)
      return Boolean(credential?.disabled)
    })

    if (disabledIds.length === 0) {
      toast.error('选中的凭据中没有已禁用项')
      return
    }

    const skippedCount = selectedIds.size - disabledIds.length
    const skippedText = skippedCount > 0 ? `（将跳过 ${skippedCount} 个未禁用凭据）` : ''

    if (!confirm(`确定要删除 ${disabledIds.length} 个已禁用凭据吗？此操作无法撤销。${skippedText}`)) {
      return
    }

    let successCount = 0
    let failCount = 0

    for (const id of disabledIds) {
      try {
        await new Promise<void>((resolve, reject) => {
          deleteCredential(id, {
            onSuccess: () => {
              successCount++
              resolve()
            },
            onError: (err) => {
              failCount++
              reject(err)
            }
          })
        })
      } catch (error) {
        // 错误已在 onError 中处理
      }
    }

    const skippedResultText = skippedCount > 0 ? `，已跳过 ${skippedCount} 个未禁用凭据` : ''

    if (failCount === 0) {
      toast.success(`成功删除 ${successCount} 个已禁用凭据${skippedResultText}`)
    } else {
      toast.warning(`删除已禁用凭据：成功 ${successCount} 个，失败 ${failCount} 个${skippedResultText}`)
    }

    deselectAll()
  }

  // 批量恢复异常
  const handleBatchResetFailure = async () => {
    if (selectedIds.size === 0) {
      toast.error('请先选择要恢复的凭据')
      return
    }

    const failedIds = Array.from(selectedIds).filter(id => {
      const cred = data?.credentials.find(c => c.id === id)
      return cred && cred.failureCount > 0
    })

    if (failedIds.length === 0) {
      toast.error('选中的凭据中没有失败的凭据')
      return
    }

    let successCount = 0
    let failCount = 0

    for (const id of failedIds) {
      try {
        await new Promise<void>((resolve, reject) => {
          resetFailure(id, {
            onSuccess: () => {
              successCount++
              resolve()
            },
            onError: (err) => {
              failCount++
              reject(err)
            }
          })
        })
      } catch (error) {
        // 错误已在 onError 中处理
      }
    }

    if (failCount === 0) {
      toast.success(`成功恢复 ${successCount} 个凭据`)
    } else {
      toast.warning(`成功 ${successCount} 个，失败 ${failCount} 个`)
    }

    deselectAll()
  }

  // 批量刷新 Token
  const handleBatchForceRefresh = async () => {
    if (selectedIds.size === 0) {
      toast.error('请先选择要刷新的凭据')
      return
    }

    const enabledIds = Array.from(selectedIds).filter(id => {
      const cred = data?.credentials.find(c => c.id === id)
      return cred && !cred.disabled
    })

    if (enabledIds.length === 0) {
      toast.error('选中的凭据中没有启用的凭据')
      return
    }

    setBatchRefreshing(true)
    setBatchRefreshProgress({ current: 0, total: enabledIds.length })

    let successCount = 0
    let failCount = 0

    for (let i = 0; i < enabledIds.length; i++) {
      try {
        await forceRefreshToken(enabledIds[i])
        successCount++
      } catch {
        failCount++
      }
      setBatchRefreshProgress({ current: i + 1, total: enabledIds.length })
    }

    setBatchRefreshing(false)
    queryClient.invalidateQueries({ queryKey: ['credentials'] })

    if (failCount === 0) {
      toast.success(`成功刷新 ${successCount} 个凭据的 Token`)
    } else {
      toast.warning(`刷新 Token：成功 ${successCount} 个，失败 ${failCount} 个`)
    }

    deselectAll()
  }

  // 一键清除所有已禁用凭据
  const handleClearAll = async () => {
    if (!data?.credentials || data.credentials.length === 0) {
      toast.error('没有可清除的凭据')
      return
    }

    const disabledCredentials = data.credentials.filter(credential => credential.disabled)

    if (disabledCredentials.length === 0) {
      toast.error('没有可清除的已禁用凭据')
      return
    }

    if (!confirm(`确定要清除所有 ${disabledCredentials.length} 个已禁用凭据吗？此操作无法撤销。`)) {
      return
    }

    let successCount = 0
    let failCount = 0

    for (const credential of disabledCredentials) {
      try {
        await new Promise<void>((resolve, reject) => {
          deleteCredential(credential.id, {
            onSuccess: () => {
              successCount++
              resolve()
            },
            onError: (err) => {
              failCount++
              reject(err)
            }
          })
        })
      } catch (error) {
        // 错误已在 onError 中处理
      }
    }

    if (failCount === 0) {
      toast.success(`成功清除所有 ${successCount} 个已禁用凭据`)
    } else {
      toast.warning(`清除已禁用凭据：成功 ${successCount} 个，失败 ${failCount} 个`)
    }

    deselectAll()
  }

  // 查询当前页凭据信息（逐个查询，避免瞬时并发）
  const handleQueryCurrentPageInfo = async () => {
    if (currentCredentials.length === 0) {
      toast.error('当前页没有可查询的凭据')
      return
    }

    const ids = currentCredentials
      .filter(credential => !credential.disabled)
      .map(credential => credential.id)

    if (ids.length === 0) {
      toast.error('当前页没有可查询的启用凭据')
      return
    }

    setQueryingInfo(true)
    setQueryInfoProgress({ current: 0, total: ids.length })

    let successCount = 0
    let failCount = 0

    for (let i = 0; i < ids.length; i++) {
      const id = ids[i]

      setLoadingBalanceIds(prev => {
        const next = new Set(prev)
        next.add(id)
        return next
      })

      try {
        const balance = await getCredentialBalance(id)
        successCount++

        setBalanceMap(prev => {
          const next = new Map(prev)
          next.set(id, balance)
          return next
        })
      } catch (error) {
        failCount++
      } finally {
        setLoadingBalanceIds(prev => {
          const next = new Set(prev)
          next.delete(id)
          return next
        })
      }

      setQueryInfoProgress({ current: i + 1, total: ids.length })
    }

    setQueryingInfo(false)
    // 后端 get_balance 已写入磁盘缓存，刷新 cached-balances 让其他卡片也能拿到最新值
    queryClient.invalidateQueries({ queryKey: ['cached-balances'] })

    if (failCount === 0) {
      toast.success(`查询完成：成功 ${successCount}/${ids.length}`)
    } else {
      toast.warning(`查询完成：成功 ${successCount} 个，失败 ${failCount} 个`)
    }
  }

  // 批量验活
  const handleBatchVerify = async () => {
    if (selectedIds.size === 0) {
      toast.error('请先选择要验活的凭据')
      return
    }

    // 初始化状态
    setVerifying(true)
    cancelVerifyRef.current = false
    const ids = Array.from(selectedIds)
    setVerifyProgress({ current: 0, total: ids.length })

    let successCount = 0

    // 初始化结果，所有凭据状态为 pending
    const initialResults = new Map<number, VerifyResult>()
    ids.forEach(id => {
      initialResults.set(id, { id, status: 'pending' })
    })
    setVerifyResults(initialResults)
    setVerifyDialogOpen(true)

    // 开始验活
    for (let i = 0; i < ids.length; i++) {
      // 检查是否取消
      if (cancelVerifyRef.current) {
        toast.info('已取消验活')
        break
      }

      const id = ids[i]

      // 更新当前凭据状态为 verifying
      setVerifyResults(prev => {
        const newResults = new Map(prev)
        newResults.set(id, { id, status: 'verifying' })
        return newResults
      })

      try {
        const balance = await getCredentialBalance(id)
        successCount++

        // 更新为成功状态
        setVerifyResults(prev => {
          const newResults = new Map(prev)
          newResults.set(id, {
            id,
            status: 'success',
            usage: `${balance.currentUsage}/${balance.usageLimit}`
          })
          return newResults
        })
      } catch (error) {
        // 更新为失败状态
        setVerifyResults(prev => {
          const newResults = new Map(prev)
          newResults.set(id, {
            id,
            status: 'failed',
            error: extractErrorMessage(error)
          })
          return newResults
        })
      }

      // 更新进度
      setVerifyProgress({ current: i + 1, total: ids.length })

      // 添加延迟防止封号（最后一个不需要延迟）
      if (i < ids.length - 1 && !cancelVerifyRef.current) {
        await new Promise(resolve => setTimeout(resolve, 2000))
      }
    }

    setVerifying(false)

    if (!cancelVerifyRef.current) {
      toast.success(`验活完成：成功 ${successCount}/${ids.length}`)
    }
  }

  // 取消验活
  const handleCancelVerify = () => {
    cancelVerifyRef.current = true
    setVerifying(false)
  }

  if (isLoading) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-background">
        <div className="text-center">
          <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary mx-auto mb-4"></div>
          <p className="text-muted-foreground">加载中...</p>
        </div>
      </div>
    )
  }

  if (error) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-background p-4">
        <Card className="w-full max-w-md p-6 text-center">
          <div className="text-red-500 mb-4">加载失败</div>
          <p className="text-muted-foreground mb-4">{(error as Error).message}</p>
          <div className="space-x-2">
            <Button onClick={() => refetch()}>重试</Button>
            <Button variant="outline" onClick={handleLogout}>重新登录</Button>
          </div>
        </Card>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-background">
      {/* 顶部导航 */}
      <header className="sticky top-0 z-50 w-full border-b bg-background/80 backdrop-blur supports-[backdrop-filter]:bg-background/60">
        <div className="container flex h-14 items-center justify-between px-4 md:px-8">
          <div className="flex items-center gap-2">
            <div className="h-7 w-7 rounded-md bg-primary/10 flex items-center justify-center">
              <Server className="h-4 w-4 text-primary" />
            </div>
            <span className="font-semibold tracking-tight">Kiro Admin</span>
          </div>
          <div className="flex items-center gap-1">
            <Button variant="ghost" size="icon" className="h-8 w-8" onClick={() => setLogsDialogOpen(true)} title="日志查看">
              <FileText className="h-4 w-4" />
            </Button>
            <Button variant="ghost" size="icon" className="h-8 w-8" onClick={() => setSettingsDialogOpen(true)} title="系统配置">
              <Settings className="h-4 w-4" />
            </Button>
            <Button variant="ghost" size="icon" className="h-8 w-8" onClick={toggleDarkMode} title={darkMode ? '浅色主题' : '深色主题'}>
              {darkMode ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
            </Button>
            <Button variant="ghost" size="icon" className="h-8 w-8" onClick={handleRefresh} title="刷新">
              <RefreshCw className="h-4 w-4" />
            </Button>
            <Button variant="ghost" size="icon" className="h-8 w-8" onClick={handleLogout} title="退出登录">
              <LogOut className="h-4 w-4" />
            </Button>
          </div>
        </div>
      </header>

      {/* 主内容 */}
      <main className="container mx-auto px-4 md:px-8 py-5">
        {/* 统计卡片 */}
        <div className="grid gap-3 md:grid-cols-3 mb-4">
          <Card className="p-3">
            <div className="text-xs font-medium text-muted-foreground mb-1">凭据总数</div>
            <div className="text-xl font-bold">{data?.total || 0}</div>
          </Card>
          <Card className="p-3">
            <div className="text-xs font-medium text-muted-foreground mb-1">可用凭据</div>
            <div className="text-xl font-bold text-green-600">{data?.available || 0}</div>
          </Card>
          <Card className="p-3">
            <div className="text-xs font-medium text-muted-foreground mb-1">当前活跃</div>
            <div className="text-xl font-bold flex items-center gap-2">
              #{data?.currentId || '-'}
              <Badge variant="success" className="h-4 px-1 text-[10px]">活跃</Badge>
            </div>
          </Card>
        </div>

        {/* 凭据管理 */}
        <Card className="p-4">
          {/* 标题行 */}
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-2">
              <h2 className="text-base font-semibold">凭据管理</h2>
              <span className="text-xs text-muted-foreground">
                {hasActiveFilter
                  ? `${sortedCredentials.length}/${data?.credentials.length ?? 0}`
                  : (data?.credentials.length ?? 0)} 个
              </span>
            </div>
            <div className="flex items-center gap-1.5">
              {data?.credentials && data.credentials.length > 0 && (
                <Button
                  onClick={handleQueryCurrentPageInfo}
                  size="sm"
                  variant="outline"
                  className="h-8 text-xs"
                  disabled={queryingInfo}
                  title="查询当前页凭据余额"
                >
                  <RefreshCw className={`h-3.5 w-3.5 mr-1 ${queryingInfo ? 'animate-spin' : ''}`} />
                  {queryingInfo ? `${queryInfoProgress.current}/${queryInfoProgress.total}` : '查询信息'}
                </Button>
              )}
              <Button
                onClick={() => setImportTokenJsonDialogOpen(true)}
                size="sm"
                variant="outline"
                className="h-8 text-xs"
              >
                <Upload className="h-3.5 w-3.5 mr-1" />
                批量导入
              </Button>
              <Button
                onClick={() => setAddDialogOpen(true)}
                size="sm"
                className="h-8 text-xs"
              >
                <Plus className="h-3.5 w-3.5 mr-1" />
                添加凭据
              </Button>
            </div>
          </div>

          {/* 左筛选 + 右动作工具栏 */}
          <div className="flex flex-col lg:flex-row gap-2 items-stretch lg:items-center mb-3 pb-3 border-b">
            {/* 左：筛选 */}
            <div className="flex items-center gap-2 flex-wrap flex-1">
              <div className="relative flex-1 min-w-[200px] max-w-xs">
                <Search className="h-3.5 w-3.5 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground" />
                <input
                  type="text"
                  placeholder="搜索 ID / 邮箱 / 端点"
                  value={searchTerm}
                  onChange={e => setSearchTerm(e.target.value)}
                  className="w-full h-8 pl-8 pr-7 text-xs border rounded-md bg-background focus:outline-none focus:ring-2 focus:ring-primary/30"
                />
                {searchTerm && (
                  <button
                    onClick={() => setSearchTerm('')}
                    className="absolute right-1.5 top-1/2 -translate-y-1/2 h-4 w-4 inline-flex items-center justify-center text-muted-foreground hover:text-foreground"
                    title="清空"
                  >
                    <X className="h-3 w-3" />
                  </button>
                )}
              </div>

              {/* 状态过滤 */}
              <div className="flex items-center gap-0.5 border rounded-md p-0.5 bg-muted/30">
                {([
                  ['all', '全部'],
                  ['enabled', '启用'],
                  ['disabled', '禁用'],
                  ['failed', '异常'],
                  ['current', '活跃'],
                ] as const).map(([k, label]) => (
                  <button
                    key={k}
                    onClick={() => setStatusFilter(k)}
                    className={`h-7 px-2 text-xs rounded transition ${
                      statusFilter === k
                        ? 'bg-background shadow-sm text-foreground font-medium'
                        : 'text-muted-foreground hover:text-foreground'
                    }`}
                  >
                    {label}
                  </button>
                ))}
              </div>

              {/* 排序 */}
              <div className="flex items-center gap-1">
                <select
                  value={sortField}
                  onChange={e => setSortField(e.target.value as SortField)}
                  className="h-8 px-2 text-xs border rounded-md bg-background focus:outline-none focus:ring-2 focus:ring-primary/30"
                  title="排序字段"
                >
                  <option value="default">默认</option>
                  <option value="id">ID</option>
                  <option value="priority">优先级</option>
                  <option value="balance">余额</option>
                  <option value="lastUsed">最后调用</option>
                </select>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8"
                  onClick={() => setSortOrder(o => o === 'asc' ? 'desc' : 'asc')}
                  disabled={sortField === 'default'}
                  title={sortOrder === 'asc' ? '升序' : '降序'}
                >
                  {sortOrder === 'asc' ? <ArrowUp className="h-3.5 w-3.5" /> : <ArrowDown className="h-3.5 w-3.5" />}
                </Button>
              </div>
            </div>

            {/* 右：批量操作快捷区（仅当有选中时显示） */}
            {selectedIds.size === 0 && data?.credentials && data.credentials.length > 0 && (
              <Button
                onClick={handleClearAll}
                size="sm"
                variant="ghost"
                className="h-8 text-xs text-destructive hover:text-destructive hover:bg-destructive/10"
                disabled={disabledCredentialCount === 0}
                title={disabledCredentialCount === 0 ? '没有可清除的已禁用凭据' : `清除 ${disabledCredentialCount} 个已禁用凭据`}
              >
                <Trash2 className="h-3.5 w-3.5 mr-1" />
                清除已禁用
              </Button>
            )}
            {verifying && !verifyDialogOpen && (
              <Button
                onClick={() => setVerifyDialogOpen(true)}
                size="sm"
                variant="secondary"
                className="h-8 text-xs"
              >
                <CheckCircle2 className="h-3.5 w-3.5 mr-1 animate-spin" />
                验活中 {verifyProgress.current}/{verifyProgress.total}
              </Button>
            )}
          </div>

          {/* 选中批量操作条 */}
          {selectedIds.size > 0 && (
            <div className="flex items-center gap-2 mb-3 p-2 rounded-md bg-primary/5 border border-primary/20">
              <Badge variant="secondary" className="h-6 px-2">已选 {selectedIds.size}</Badge>
              <div className="flex items-center gap-1">
                <Button onClick={handleBatchVerify} size="sm" variant="ghost" className="h-7 text-xs">
                  <CheckCircle2 className="h-3.5 w-3.5 mr-1" />批量验活
                </Button>
                <Button
                  onClick={handleBatchForceRefresh}
                  size="sm"
                  variant="ghost"
                  className="h-7 text-xs"
                  disabled={batchRefreshing}
                >
                  <RefreshCw className={`h-3.5 w-3.5 mr-1 ${batchRefreshing ? 'animate-spin' : ''}`} />
                  {batchRefreshing ? `刷新 ${batchRefreshProgress.current}/${batchRefreshProgress.total}` : '批量刷新'}
                </Button>
                <Button onClick={handleBatchResetFailure} size="sm" variant="ghost" className="h-7 text-xs">
                  <RotateCcw className="h-3.5 w-3.5 mr-1" />恢复异常
                </Button>
                <Button
                  onClick={handleBatchDelete}
                  size="sm"
                  variant="ghost"
                  className="h-7 text-xs text-destructive hover:text-destructive hover:bg-destructive/10"
                  disabled={selectedDisabledCount === 0}
                  title={selectedDisabledCount === 0 ? '只能删除已禁用凭据' : undefined}
                >
                  <Trash2 className="h-3.5 w-3.5 mr-1" />批量删除
                </Button>
              </div>
              <div className="flex-1" />
              <Button onClick={deselectAll} size="sm" variant="ghost" className="h-7 text-xs">
                取消选择
              </Button>
            </div>
          )}

          {/* 凭据卡片网格 */}
          {data?.credentials.length === 0 ? (
            <div className="py-12 text-center text-sm text-muted-foreground">
              暂无凭据，点击右上"添加凭据"或"批量导入"开始
            </div>
          ) : sortedCredentials.length === 0 ? (
            <div className="py-12 text-center text-sm text-muted-foreground">
              没有匹配当前过滤条件的凭据
              <div className="mt-2">
                <Button
                  size="sm"
                  variant="outline"
                  className="h-7 text-xs"
                  onClick={() => { setSearchTerm(''); setStatusFilter('all') }}
                >
                  清除过滤
                </Button>
              </div>
            </div>
          ) : (
            <>
              <div className="grid gap-2.5 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
                {currentCredentials.map((credential) => (
                  <CredentialCard
                    key={credential.id}
                    credential={credential}
                    onViewBalance={handleViewBalance}
                    selected={selectedIds.has(credential.id)}
                    onToggleSelect={() => toggleSelect(credential.id)}
                    balance={balanceMap.get(credential.id) || null}
                    loadingBalance={loadingBalanceIds.has(credential.id)}
                    cachedBalance={cachedBalanceMap.get(credential.id)}
                  />
                ))}
              </div>

              {/* 分页 */}
              {totalPages > 1 && (
                <div className="flex justify-center items-center gap-3 mt-4 pt-3 border-t">
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-7 text-xs"
                    onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
                    disabled={currentPage === 1}
                  >
                    上一页
                  </Button>
                  <span className="text-xs text-muted-foreground">
                    第 {currentPage} / {totalPages} 页 · 共 {sortedCredentials.length} 个
                  </span>
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-7 text-xs"
                    onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}
                    disabled={currentPage === totalPages}
                  >
                    下一页
                  </Button>
                </div>
              )}
            </>
          )}
        </Card>
      </main>

      {/* 余额对话框 */}
      <BalanceDialog
        credentialId={selectedCredentialId}
        open={balanceDialogOpen}
        onOpenChange={setBalanceDialogOpen}
      />

      {/* 添加凭据对话框 */}
      <AddCredentialDialog
        open={addDialogOpen}
        onOpenChange={setAddDialogOpen}
      />

      {/* 批量导入 Token JSON 对话框 */}
      <ImportTokenJsonDialog
        open={importTokenJsonDialogOpen}
        onOpenChange={setImportTokenJsonDialogOpen}
      />

      {/* 系统配置对话框（阶段 7） */}
      <SettingsDialog
        open={settingsDialogOpen}
        onOpenChange={setSettingsDialogOpen}
      />

      {/* 日志查看对话框（阶段 7.9） */}
      <LogsDialog
        open={logsDialogOpen}
        onOpenChange={setLogsDialogOpen}
      />

      {/* 批量验活对话框 */}
      <BatchVerifyDialog
        open={verifyDialogOpen}
        onOpenChange={setVerifyDialogOpen}
        verifying={verifying}
        progress={verifyProgress}
        results={verifyResults}
        onCancel={handleCancelVerify}
      />
    </div>
  )
}
