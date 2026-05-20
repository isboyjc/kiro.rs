import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  getCredentials,
  setCredentialDisabled,
  setCredentialPriority,
  setCredentialEndpoint,
  setCredentialRegion,
  resetCredentialFailure,
  forceRefreshToken,
  getCredentialBalance,
  getCredentialModels,
  getCachedBalances,
  addCredential,
  deleteCredential,
  getLoadBalancingMode,
  setLoadBalancingMode,
  importTokenJson,
} from '@/api/credentials'
import type { AddCredentialRequest, ImportTokenJsonRequest } from '@/types/api'

// 查询凭据列表
export function useCredentials() {
  return useQuery({
    queryKey: ['credentials'],
    queryFn: getCredentials,
    refetchInterval: 30000, // 每 30 秒刷新一次
  })
}

// 查询凭据余额
export function useCredentialBalance(id: number | null) {
  return useQuery({
    queryKey: ['credential-balance', id],
    queryFn: () => getCredentialBalance(id!),
    enabled: id !== null,
    retry: false, // 余额查询失败时不重试（避免重复请求被封禁的账号）
  })
}

// 阶段 7.16：查询凭据可用模型列表
export function useCredentialModels(id: number | null) {
  return useQuery({
    queryKey: ['credential-models', id],
    queryFn: () => getCredentialModels(id!),
    enabled: id !== null,
    retry: false,
    staleTime: 5 * 60 * 1000, // 模型列表变化慢，缓存 5 分钟
  })
}

// 查询所有凭据的缓存余额（轻量、纯读后端缓存、不触发号池上游请求）
export function useCachedBalances() {
  return useQuery({
    queryKey: ['cached-balances'],
    queryFn: getCachedBalances,
    refetchInterval: 30000, // 每 30 秒拉取一次
    staleTime: 10000,
  })
}

// 设置禁用状态
export function useSetDisabled() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ id, disabled }: { id: number; disabled: boolean }) =>
      setCredentialDisabled(id, disabled),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['credentials'] })
    },
  })
}

// 设置优先级
export function useSetPriority() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ id, priority }: { id: number; priority: number }) =>
      setCredentialPriority(id, priority),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['credentials'] })
    },
  })
}

// 设置 endpoint
export function useSetEndpoint() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ id, endpoint }: { id: number; endpoint: string | null }) =>
      setCredentialEndpoint(id, endpoint),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['credentials'] })
    },
  })
}

// 设置 Region / API Region
export function useSetRegion() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ id, region, apiRegion }: { id: number; region: string | null; apiRegion: string | null }) =>
      setCredentialRegion(id, region, apiRegion),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['credentials'] })
    },
  })
}

// 重置失败计数
export function useResetFailure() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (id: number) => resetCredentialFailure(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['credentials'] })
    },
  })
}

// 强制刷新 Token
export function useForceRefreshToken() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (id: number) => forceRefreshToken(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['credentials'] })
    },
  })
}

// 添加新凭据
export function useAddCredential() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (req: AddCredentialRequest) => addCredential(req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['credentials'] })
    },
  })
}

// 删除凭据
export function useDeleteCredential() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (id: number) => deleteCredential(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['credentials'] })
    },
  })
}

// 获取负载均衡模式
export function useLoadBalancingMode() {
  return useQuery({
    queryKey: ['loadBalancingMode'],
    queryFn: getLoadBalancingMode,
  })
}

// 设置负载均衡模式
export function useSetLoadBalancingMode() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: setLoadBalancingMode,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['loadBalancingMode'] })
    },
  })
}

// === 阶段 5.3 批量导入 hook ===
export function useImportTokenJson() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (req: ImportTokenJsonRequest) => importTokenJson(req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['credentials'] })
    },
  })
}
