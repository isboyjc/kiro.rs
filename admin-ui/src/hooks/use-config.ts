import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { getConfig, getConfigRaw, updateConfig, validateConfig } from '@/api/config'
import type { ConfigJson } from '@/types/api'

/** 拉取当前结构化 Config（不轮询，用户主动打开 Dialog 时触发） */
export function useConfig(enabled: boolean = true) {
  return useQuery({
    queryKey: ['config'],
    queryFn: getConfig,
    enabled,
    staleTime: 5000,
  })
}

/** 拉取 raw config.json 文本 */
export function useConfigRaw(enabled: boolean = true) {
  return useQuery({
    queryKey: ['config-raw'],
    queryFn: getConfigRaw,
    enabled,
    staleTime: 5000,
  })
}

/** 校验（前端 debounce 后调用） */
export function useValidateConfig() {
  return useMutation({
    mutationFn: (req: ConfigJson) => validateConfig(req),
  })
}

/** 保存 + 写盘 + 热生效 */
export function useUpdateConfig() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (req: ConfigJson) => updateConfig(req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['config'] })
      queryClient.invalidateQueries({ queryKey: ['config-raw'] })
      // 其他可能受影响的 query
      queryClient.invalidateQueries({ queryKey: ['loadBalancingMode'] })
    },
  })
}
