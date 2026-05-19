// 凭据状态响应
export interface CredentialsStatusResponse {
  total: number
  available: number
  currentId: number
  credentials: CredentialStatusItem[]
}

// 单个凭据状态
export interface CredentialStatusItem {
  id: number
  priority: number
  disabled: boolean
  failureCount: number
  isCurrent: boolean
  expiresAt: string | null
  authMethod: string | null
  hasProfileArn: boolean
  email?: string
  refreshTokenHash?: string
  apiKeyHash?: string
  maskedApiKey?: string
  successCount: number
  lastUsedAt: string | null
  hasProxy: boolean
  proxyUrl?: string
  refreshFailureCount: number
  disabledReason?: string
  endpoint: string
}

// === 阶段 7：Config 面板 ===
// 完整 Config 结构（与后端 src/model/config.rs 的 Config 对齐，camelCase）
// 为了允许前端自由编辑（含未来扩展字段），用 Record<string, unknown> 而非强类型
export type ConfigJson = Record<string, unknown>

export interface ConfigRawResponse {
  content: string
  path: string
}

export interface ConfigFieldError {
  path: string
  message: string
}

export interface ConfigValidateResponse {
  valid: boolean
  errors: ConfigFieldError[]
  needsRestart: string[]
  hotReload: string[]
}

export interface ConfigUpdateResponse {
  ok: boolean
  message: string
  needsRestart: string[]
  hotReload: string[]
  /** adminApiKey 被修改时返回新值，前端用于自动重连 */
  newAdminApiKey?: string
  /** apiKey 被修改时返回新值，便于前端展示提示客户端 */
  newApiKey?: string
}

export type ConfigFieldType = 'string' | 'number' | 'boolean' | 'enum'

export interface ConfigSchemaEnumOption {
  value: string
  label: string
}

export interface ConfigSchemaField {
  /** 点号路径：`compression.enabled` 或 `host` */
  key: string
  label: string
  type: ConfigFieldType
  needsRestart: boolean
  sensitive: boolean
  nullable: boolean
  description?: string
  warning?: string
  defaultValue?: unknown
  min?: number
  max?: number
  enumOptions?: ConfigSchemaEnumOption[]
  placeholder?: string
}

export interface ConfigSchemaGroup {
  id: string
  label: string
  description?: string
  needsRestart: boolean
  sensitive: boolean
  fields: ConfigSchemaField[]
}

export interface ConfigSchemaResponse {
  groups: ConfigSchemaGroup[]
}

// 缓存余额条目（来自 GET /credentials/balances/cached，纯磁盘缓存快照）
export interface CachedBalanceInfo {
  id: number
  remaining: number
  usageLimit: number
  usagePercentage: number
  subscriptionTitle: string | null
  cachedAt: number // Unix 毫秒
  ttlSecs: number
}

// 所有凭据的缓存余额响应
export interface CachedBalancesResponse {
  balances: CachedBalanceInfo[]
}

// 余额响应
export interface BalanceResponse {
  id: number
  subscriptionTitle: string | null
  currentUsage: number
  usageLimit: number
  remaining: number
  usagePercentage: number
  nextResetAt: number | null
}

// 成功响应
export interface SuccessResponse {
  success: boolean
  message: string
}

// 错误响应
export interface AdminErrorResponse {
  error: {
    type: string
    message: string
  }
}

// 请求类型
export interface SetDisabledRequest {
  disabled: boolean
}

export interface SetPriorityRequest {
  priority: number
}

// 添加凭据请求
export interface AddCredentialRequest {
  refreshToken?: string
  authMethod?: 'social' | 'idc' | 'api_key'
  clientId?: string
  clientSecret?: string
  priority?: number
  authRegion?: string
  apiRegion?: string
  machineId?: string
  proxyUrl?: string
  proxyUsername?: string
  proxyPassword?: string
  kiroApiKey?: string
  endpoint?: string
}

// 添加凭据响应
export interface AddCredentialResponse {
  success: boolean
  message: string
  credentialId: number
  email?: string
}

// === 阶段 5.3a 批量导入 token.json 类型 ===

export interface TokenJsonItem {
  provider?: string
  refreshToken?: string
  clientId?: string
  clientSecret?: string
  authMethod?: string
  priority?: number
  region?: string
  apiRegion?: string
  machineId?: string
}

export interface ImportTokenJsonRequest {
  dryRun?: boolean
  items: TokenJsonItem | TokenJsonItem[]
}

export type ImportAction = 'added' | 'skipped' | 'invalid'

export interface ImportItemResult {
  index: number
  fingerprint: string
  action: ImportAction
  reason?: string
  credentialId?: number
}

export interface ImportSummary {
  parsed: number
  added: number
  skipped: number
  invalid: number
}

export interface ImportTokenJsonResponse {
  summary: ImportSummary
  items: ImportItemResult[]
}
