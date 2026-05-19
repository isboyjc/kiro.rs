import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { clearLogs, getLogs } from '@/api/logs'
import type { LogsQueryParams } from '@/types/api'

/**
 * 日志查询 hook
 * - autoRefresh: 是否每 3s 轮询
 * - enabled: 是否触发（关闭 dialog 时设 false）
 */
export function useLogs(params: LogsQueryParams, enabled: boolean, autoRefresh: boolean) {
  return useQuery({
    queryKey: ['admin-logs', params],
    queryFn: () => getLogs(params),
    enabled,
    refetchInterval: autoRefresh ? 3000 : false,
    staleTime: 1000,
  })
}

export function useClearLogs() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => clearLogs(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-logs'] })
    },
  })
}
