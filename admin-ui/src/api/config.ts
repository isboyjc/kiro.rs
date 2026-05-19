import axios from 'axios'
import { storage } from '@/lib/storage'
import type {
  ConfigJson,
  ConfigRawResponse,
  ConfigValidateResponse,
  ConfigUpdateResponse,
} from '@/types/api'

const api = axios.create({
  baseURL: '/api/admin',
  headers: { 'Content-Type': 'application/json' },
})

api.interceptors.request.use((config) => {
  const apiKey = storage.getApiKey()
  if (apiKey) {
    config.headers['x-api-key'] = apiKey
  }
  return config
})

/** GET /api/admin/config — 结构化配置 */
export async function getConfig(): Promise<ConfigJson> {
  const { data } = await api.get<ConfigJson>('/config')
  return data
}

/** GET /api/admin/config/raw — 原始 JSON 文本 */
export async function getConfigRaw(): Promise<ConfigRawResponse> {
  const { data } = await api.get<ConfigRawResponse>('/config/raw')
  return data
}

/** POST /api/admin/config/validate — 仅校验，不写盘 */
export async function validateConfig(
  newConfig: ConfigJson
): Promise<ConfigValidateResponse> {
  const { data } = await api.post<ConfigValidateResponse>('/config/validate', newConfig)
  return data
}

/** PUT /api/admin/config — 全量替换 + 写盘 + 投射热生效字段 */
export async function updateConfig(
  newConfig: ConfigJson
): Promise<ConfigUpdateResponse> {
  const { data } = await api.put<ConfigUpdateResponse>('/config', newConfig)
  return data
}
