import axios from 'axios'
import { storage } from '@/lib/storage'
import type { LogsResponse, LogsQueryParams } from '@/types/api'

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

export async function getLogs(params: LogsQueryParams): Promise<LogsResponse> {
  const { data } = await api.get<LogsResponse>('/logs', { params })
  return data
}

export async function clearLogs(): Promise<void> {
  await api.delete('/logs')
}
