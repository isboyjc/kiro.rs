import { useState } from 'react'
import { AlertCircle, Eye, EyeOff } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { Button } from '@/components/ui/button'
import type {
  ConfigJson,
  ConfigSchemaField,
  ConfigSchemaGroup,
  ConfigSchemaResponse,
} from '@/types/api'

interface ConfigFormProps {
  schema: ConfigSchemaResponse
  /** 当前编辑中的 Config 对象 */
  value: ConfigJson
  /** 字段路径 → 新值；form 内部直接 mutate value 并通过此回调上抛 */
  onChange: (next: ConfigJson) => void
}

/** 点号路径读取嵌套值 */
function getByPath(obj: ConfigJson, path: string): unknown {
  return path.split('.').reduce<unknown>((acc, key) => {
    if (acc && typeof acc === 'object' && key in (acc as Record<string, unknown>)) {
      return (acc as Record<string, unknown>)[key]
    }
    return undefined
  }, obj)
}

/** 点号路径写入嵌套值（克隆 + 不可变写入） */
function setByPath(obj: ConfigJson, path: string, value: unknown): ConfigJson {
  const keys = path.split('.')
  const result = { ...obj }
  let cursor: Record<string, unknown> = result as Record<string, unknown>
  for (let i = 0; i < keys.length - 1; i++) {
    const k = keys[i]
    const existing = cursor[k]
    const cloned =
      existing && typeof existing === 'object' && !Array.isArray(existing)
        ? { ...(existing as Record<string, unknown>) }
        : {}
    cursor[k] = cloned
    cursor = cloned as Record<string, unknown>
  }
  cursor[keys[keys.length - 1]] = value
  return result
}

export function ConfigForm({ schema, value, onChange }: ConfigFormProps) {
  return (
    <div className="space-y-3 max-h-[500px] overflow-y-auto pr-1 -mr-1">
      {schema.groups.map((group) => (
        <GroupCard key={group.id} group={group} value={value} onChange={onChange} />
      ))}
    </div>
  )
}

function GroupCard({
  group,
  value,
  onChange,
}: {
  group: ConfigSchemaGroup
  value: ConfigJson
  onChange: (next: ConfigJson) => void
}) {
  return (
    <div className="rounded-lg border bg-card overflow-hidden">
      <div className="flex items-center justify-between px-3 py-2 bg-muted/30">
        <div className="flex items-center gap-2 min-w-0">
          <GroupDot needsRestart={group.needsRestart} sensitive={group.sensitive} />
          <div className="min-w-0">
            <div className="font-medium text-sm">{group.label}</div>
            {group.description && (
              <div className="text-[11px] text-muted-foreground mt-0.5 truncate">{group.description}</div>
            )}
          </div>
        </div>
        <GroupBadge needsRestart={group.needsRestart} sensitive={group.sensitive} />
      </div>
      <div className="p-3 space-y-2.5">
        {group.fields.map((f) => (
          <FieldRow
            key={f.key}
            field={f}
            value={getByPath(value, f.key)}
            onChange={(v) => onChange(setByPath(value, f.key, v))}
          />
        ))}
      </div>
    </div>
  )
}

function GroupDot({ needsRestart, sensitive }: { needsRestart: boolean; sensitive: boolean }) {
  const color = sensitive ? 'bg-red-500' : needsRestart ? 'bg-amber-500' : 'bg-green-500'
  return <span className={`h-2 w-2 rounded-full ${color} shrink-0`} />
}

function GroupBadge({ needsRestart, sensitive }: { needsRestart: boolean; sensitive: boolean }) {
  if (sensitive) {
    return <Badge variant="destructive" className="h-4 px-1.5 text-[10px]">敏感</Badge>
  }
  if (needsRestart) {
    return <Badge variant="outline" className="h-4 px-1.5 text-[10px] text-amber-600 border-amber-300">需重启</Badge>
  }
  return <Badge variant="secondary" className="h-4 px-1.5 text-[10px]">热生效</Badge>
}

function FieldRow({
  field,
  value,
  onChange,
}: {
  field: ConfigSchemaField
  value: unknown
  onChange: (v: unknown) => void
}) {
  const [showSensitive, setShowSensitive] = useState(false)

  return (
    <div className="grid grid-cols-12 gap-3 items-center">
      {/* 标签列 */}
      <div className="col-span-4 min-w-0">
        <div className="text-xs font-medium flex items-center gap-1 truncate">
          {field.label}
          {field.needsRestart && !field.sensitive && (
            <span className="h-1.5 w-1.5 rounded-full bg-amber-500" title="需重启生效" />
          )}
          {field.sensitive && (
            <span className="h-1.5 w-1.5 rounded-full bg-red-500" title="敏感字段" />
          )}
        </div>
        <div className="font-mono text-[10px] text-muted-foreground/80 truncate" title={field.key}>{field.key}</div>
      </div>

      {/* 输入列 */}
      <div className="col-span-8 space-y-1">
        <div className="flex items-start gap-1">
          <div className="flex-1">
            <FieldInput
              field={field}
              value={value}
              onChange={onChange}
              revealed={showSensitive}
            />
          </div>
          {field.sensitive && field.type === 'string' && (
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="h-9 w-9 shrink-0"
              onClick={() => setShowSensitive((v) => !v)}
              title={showSensitive ? '隐藏' : '显示'}
            >
              {showSensitive ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
            </Button>
          )}
        </div>
        {(field.description || field.warning) && (
          <div className="space-y-0.5">
            {field.description && (
              <div className="text-[11px] text-muted-foreground leading-tight">{field.description}</div>
            )}
            {field.warning && (
              <div className="text-[11px] flex items-start gap-1 text-amber-600 leading-tight">
                <AlertCircle className="h-3 w-3 mt-0.5 shrink-0" />
                <span>{field.warning}</span>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  )
}

function FieldInput({
  field,
  value,
  onChange,
  revealed,
}: {
  field: ConfigSchemaField
  value: unknown
  onChange: (v: unknown) => void
  revealed: boolean
}) {
  if (field.type === 'boolean') {
    return (
      <div className="flex items-center gap-2">
        <Switch
          checked={Boolean(value)}
          onCheckedChange={(checked) => onChange(checked)}
        />
        <span className="text-xs text-muted-foreground">
          {Boolean(value) ? '已启用' : '已禁用'}
        </span>
      </div>
    )
  }

  if (field.type === 'enum') {
    // 若 defaultValue 是数字（如 promptCacheTtlSeconds 的 300/3600），存储为数字
    const isNumeric = typeof field.defaultValue === 'number'
    const stringValue = value == null ? '' : String(value)
    return (
      <select
        className="w-full h-9 px-2 text-sm border rounded-md bg-background focus:outline-none focus:ring-2 focus:ring-primary/40"
        value={stringValue}
        onChange={(e) => onChange(isNumeric ? Number(e.target.value) : e.target.value)}
      >
        {field.enumOptions?.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
    )
  }

  if (field.type === 'number') {
    return (
      <Input
        type="number"
        min={field.min}
        max={field.max}
        placeholder={field.placeholder}
        value={typeof value === 'number' ? value : ''}
        onChange={(e) => {
          const raw = e.target.value
          if (raw === '') {
            onChange(field.nullable ? null : 0)
          } else {
            const n = Number(raw)
            onChange(Number.isNaN(n) ? 0 : n)
          }
        }}
        className="h-9"
      />
    )
  }

  // string
  const inputType = field.sensitive && !revealed ? 'password' : 'text'
  return (
    <Input
      type={inputType}
      placeholder={field.placeholder}
      value={typeof value === 'string' ? value : value == null ? '' : String(value)}
      onChange={(e) => {
        const v = e.target.value
        // nullable + 空串 → null
        if (field.nullable && v === '') {
          onChange(null)
        } else {
          onChange(v)
        }
      }}
      className="h-9 font-mono text-xs"
    />
  )
}
