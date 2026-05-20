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
  /** 凭据级 endpoint（未配置则缺省，回退到 effectiveEndpoint） */
  endpoint?: string
  /** 实际生效的 endpoint（含默认值回退后） */
  effectiveEndpoint: string
  /** 凭据级 Region（未配置则缺省） */
  region?: string
  /** 凭据级 API Region（未配置则缺省） */
  apiRegion?: string
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

// === 阶段 7.9：日志面板 ===

export type LogKind = 'generic' | 'model_call'

export interface ModelCallMeta {
  credentialId: number
  model?: string
  endpoint: string
  apiType: string
  status: number
  durationMs: number
  retryAttempt: number
  isStream: boolean
  errorSummary?: string
  /** 阶段 7.15：token 明细（成功调用时有） */
  inputTokens?: number
  outputTokens?: number
  cacheReadInputTokens?: number
  cacheCreationInputTokens?: number
}

export interface LogEntry {
  /** 单调递增唯一 ID，前端用作 React key + 展开状态键 */
  seq: number
  timestamp: number // Unix ms
  level: string // INFO / WARN / ERROR
  kind: LogKind
  target: string
  message: string
  fields: Record<string, string>
  modelCall?: ModelCallMeta
}

export interface ModelCallStats {
  windowMs: number
  total: number
  success: number
  failed: number
  avgMs: number
  p95Ms: number
}

export interface LogsResponse {
  entries: LogEntry[]
  totalBuffered: number
  capacity: number
  stats: ModelCallStats
}

export interface LogsQueryParams {
  kind?: LogKind | 'all'
  /** 逗号分隔，如 "WARN,ERROR" */
  levels?: string
  q?: string
  credentialId?: number
  model?: string
  status?: number
  onlyFailed?: boolean
  since?: number
  limit?: number
}

// 缓存余额条目（来自 GET /credentials/balances/cached，纯磁盘缓存快照）
export interface CachedBalanceInfo {
  id: number
  /** 阶段 7.12：可能为负数 */
  remaining: number
  usageLimit: number
  /** 可超过 100% */
  usagePercentage: number
  subscriptionTitle: string | null
  cachedAt: number // Unix 毫秒
  ttlSecs: number
  /** 阶段 7.12：是否处于超额区 */
  isOverage?: boolean
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
  /** 阶段 7.12：可能为负数（表示已超额 |remaining|） */
  remaining: number
  /** 可超过 100% */
  usagePercentage: number
  nextResetAt: number | null
  /** 阶段 7.12：是否处于超额区（current >= limit 且 limit > 0） */
  isOverage?: boolean
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
