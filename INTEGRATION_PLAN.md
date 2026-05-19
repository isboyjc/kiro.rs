# kiro.rs Fork 整合计划（方案 B）

> 本文档记录将社区 fork `M-JYuan/kiro.rs` 的功能逐步整合到本仓库的实施方案。
>
> 工作分支：`work/integrate-feature`
> 基线分支：`master`（与 `upstream/master` 同步）
> 目标策略：以 upstream 为基底，按阶段 cherry-pick 移植 feature 的核心模块，便于后续持续同步主仓库。

---

## 一、背景

### 三方仓库关系

| Remote | 仓库 | 角色 | HEAD（调研时刻） |
|---|---|---|---|
| `upstream` | `hank9999/kiro.rs` | 主仓库（官方） | `f1bbe9f` |
| `origin` | `isboyjc/kiro.rs` | 本人 fork | `f1bbe9f`（与 upstream 同步） |
| `feature` | `M-JYuan/kiro.rs` | 社区衍生 fork | `4ecd10c` |

`upstream/master` 与 `feature/master` 的共同祖先（merge-base）：`1fbc529`。

### 差异规模

- `feature/master` 独有真实功能 commits（已过滤 chore/docs/style）：**约 173 个**
- `upstream/master` 独有真实功能 commits：**约 43 个**
- 双方都改动过的热点文件：**41 个**（涵盖 `kiro/provider.rs`、`token_manager.rs`、`anthropic/converter.rs`、`handlers.rs`、`stream.rs`、`admin-ui/*` 等）

### feature 独有的新文件（共 16 个）

```
src/anthropic/cache_tracker.rs
src/anthropic/compressor.rs
src/anthropic/tool_compression.rs
src/anthropic/truncation.rs
src/common/redact.rs
src/common/utf8.rs
src/image.rs
src/kiro/affinity.rs
src/kiro/background_refresh.rs
src/kiro/cooldown.rs
src/kiro/endpoint/cli.rs
src/kiro/fingerprint.rs
src/kiro/model/events/metering.rs
src/kiro/model/events/reasoning.rs
src/kiro/rate_limiter.rs
src/kiro/web_portal.rs
```

`upstream/master` **无独占新文件**——所有差异都在共有文件的修改里。

---

## 二、按领域的功能差异对比

### 1. 凭据/认证

| feature 多出 | upstream 多出 |
|---|---|
| 凭据级速率限制（`rate_limiter.rs` 484 行） | **KIRO_API_KEY 全链路支持**（`fbc5f4b` 等） |
| 用户亲和性绑定 + 余额感知故障转移 | API Key 脱敏展示、Admin UI/API |
| 启动时自动禁用余额不足凭据 | 独立 hash 字段 + 互斥 machine_id |
| 同优先级负载均衡算法 | `invalid_grant` 立即禁用 |
| 429 不冻结凭据，靠 cooldown 退避 | profileArn 由 provider 动态注入 |
| 凭据冷却快速 429+Retry-After | Endpoint trait 抽象（IDE/CLI 切换） |

**合并难度：高** ⚠️。两边在 `provider.rs`/`token_manager.rs` 上都有深度修改，需要逐处手动调和。

### 2. Prompt Cache（feature 核心创新）

feature 提供 `cache_tracker.rs`（940 行全新模块）：跨对话轮次的本地 prompt cache 追踪、多 TTL（5m / 1h）拆分、token 级精确计费、`/cc/v1/models` endpoint。

upstream **无独有 cache 功能**。

**合并难度：低**（无冲突），集成风险中（需验证与最新 Anthropic API 契约一致）。

### 3. 图片处理与压缩（feature 独占）

`image.rs`（564 行）：token 估算、自适应压缩、GIF 抽帧。
`compressor.rs`：四层自适应压缩管道（含 tool_use/tool_result 配对修复）。
单图像素限制 1.15M → 4M。

**合并难度：低**。

### 4. Admin UI / 凭据管理

| feature 多出 | upstream 多出 |
|---|---|
| 多凭据 JSON 批量导入 | API Key 凭据 Admin UI |
| 全局配置热加载（region/RPM/proxy/compression） | KAM 新平铺格式导入 |
| 凭据级 endpoint 切换、defaultEndpoint 暴露 | |
| Prompt cache 全局控制、KAM 1.8.3 集成 | |

**合并难度：中**。需协调"热加载"与"新凭据类型"。

### 5. 网络/构建（upstream 部署友好）

upstream 多出 **Musl 静态编译**、**Vendored OpenSSL**、**Rustls + 双 CA 信任**、reqwest 特性补回。
feature 多出 TCP keepalive 配置。
各自独立，**合并难度：低**。

### 6. 模型支持

upstream 的 **Opus 4.7 完整支持**（含模型映射和 1M 上下文）比 feature 的别名实现更权威，采用 upstream 版本。

### 7. 诊断工具（feature 独占）

`tools/diagnose_improper_request.py`、`replay_demo_request.py`、`test_prompt_cache_usage.mjs`、`analyze_compression.py`、`common/redact.rs`、`sensitive-logs` feature flag。

**合并难度：低**。

### 8. Web Search

feature 多出：混合工具列表中 web_search 识别 + 非流式响应处理、cache 在 fallback 中保留。**合并难度：低**。

---

## 三、Feature 独占功能清单（合并的核心价值）

1. **本地 Prompt Cache 追踪系统** — `cache_tracker.rs`
2. **完整图片/GIF 压缩方案** — `image.rs`、`compressor.rs`
3. **凭据级速率限制与负载均衡** — `rate_limiter.rs`、`affinity.rs`
4. **诊断工具集** — `tools/*.py`、`.mjs`
5. **Admin UI 配置热加载** — region/RPM/proxy/cache 运行时切换
6. **凭据冷却与后台刷新** — `cooldown.rs`、`background_refresh.rs`
7. **Web Portal API** — `web_portal.rs`
8. **日志脱敏** — `common/redact.rs`、sensitive-logs feature flag

## 四、Upstream 独占功能清单（保留 master 的价值）

1. **KIRO_API_KEY 原生支持**（Headless 认证完整链路）
2. **Musl 静态编译 + Vendored OpenSSL**（生产部署友好）
3. **Rustls 双 CA 信任**
4. **精细 Token 刷新**（`invalid_grant` 快速禁用、profileArn 动态注入）
5. **Endpoint Trait 抽象**（凭据级 IDE/CLI 切换）
6. **KAM 1.8.3 新格式兼容**
7. **模型版本与上下文窗口定期更新**

## 五、高冲突预警

| 文件 | 冲突类型 | feature / upstream 改动规模 |
|---|---|---|
| `src/kiro/provider.rs` | **语义级**（架构思路根本不同） | +1977 / +467 |
| `src/kiro/token_manager.rs` | **功能重叠**（保守 vs 自愈） | +3141 / +953 |
| `src/anthropic/converter.rs` | **代码规模冲突** | +2382 / +310 |
| `src/anthropic/handlers.rs` | **代码规模冲突** | +1940 / +148 |
| `src/anthropic/stream.rs` | **实现路径冲突** | +1195 / +136 |
| `src/model/config.rs` | **schema 冲突** | 配置项交叉新增 |
| `admin-ui/src/components/*.tsx` | **UI 逻辑冲突** | 热加载 vs 新凭据类型 |

---

## 六、整合策略：方案 B

> 以 upstream 为基础，按阶段 cherry-pick 移植 feature 的核心模块。

### 总体原则

1. **每阶段一个 commit/PR**，独立可合并、可回滚
2. **以 upstream master 为基底**——后续同步主仓库只需正常 `git merge upstream/master`
3. **不动 upstream 的核心架构**（API Key 链路、endpoint trait、Token 精细化）
4. **冲突文件优先抄逻辑而非抄代码**——在 upstream 实现上加挂载点，而不是用 feature 文件覆盖

### 准备工作

```bash
git tag backup/before-feature-integration work/integrate-feature
git checkout work/integrate-feature
```

---

## 七、五阶段迁移计划

### 阶段 1：纯工具模块（零冲突）

**范围**：

| 文件 | 行数 | 集成方式 |
|---|---|---|
| `src/common/utf8.rs` | 46 | 直接复制 + `common/mod.rs` 加 `pub mod utf8;` |
| `src/common/redact.rs` | 97 | 直接复制 + 模块声明 |
| `src/anthropic/truncation.rs` | 282 | 直接复制 + `anthropic/mod.rs` 加 `mod truncation;` |
| `src/anthropic/tool_compression.rs` | 276 | 直接复制 + 模块声明 |

**适配工作**：仅模块声明 + 头部 `#![allow(dead_code)]`（因阶段 1 尚无 caller）。

**验证**：`cargo check` 与 `cargo test` 通过。无行为变更。

**预估**：0.5 天。

### 阶段 2：CLI Endpoint + Trait 签名升级

**目标**：把 feature 的 `cli.rs` endpoint 接入 upstream 的 endpoint trait 注册表。

**两侧仓库现状差异**：

1. **Trait 签名**：upstream 的 `transform_api_body` 返回 `String`，feature 返回 `anyhow::Result<String>`；feature 还新增了 `usage_request_parts` 方法 + `UsageRequestParts` struct，upstream 无。
2. **`IdeEndpoint` 实现**：feature 版已包含 SSO OIDC 凭据（`builder-id`/`idc`）的 profileArn 区分逻辑（功能上是 upstream 的超集）。
3. **`CliEndpoint`**：feature 独占，字节级对齐 kiro-cli 2.3.0 抓包（AWS JSON 1.0 framing、KIRO_CLI origin、context entry 包装、envState 注入、wire-order 重整）。
4. **`ToolSpecification` 字段顺序** ⚠️：feature 把 `input_schema` 调到了 `name`/`description` 之前以匹配 kiro-cli 2.3.0 wire，upstream 是自然顺序——CLI endpoint 对此敏感。

**采用方式 A**：升级 upstream 的 trait 到 `anyhow::Result<String>`，一次性引入 `UsageRequestParts`（即便当前无 caller，阶段 4 重构 token_manager 时可直接用）。

| 文件 | 操作 |
|---|---|
| `src/kiro/endpoint/mod.rs` | trait 签名升级；新增 `UsageRequestParts` struct 与 `usage_request_parts` 方法；`pub mod cli;` + `pub use cli::{CLI_ENDPOINT_NAME, CliEndpoint};` |
| `src/kiro/endpoint/ide.rs` | 用 feature 版本主体替换；保留 upstream 4 个 `inject_profile_arn` 单元测试并适配新 Result 签名 |
| `src/kiro/endpoint/cli.rs` | 从 feature 完整复制（~340 行） |
| `src/kiro/model/requests/tool.rs` | **同步 feature 的 `ToolSpecification` 字段顺序**（`input_schema` 前置）；保留 upstream 现有所有测试 |
| `src/kiro/provider.rs` (2 处) | `transform_*_body` 调用加 `?` 或 `unwrap_or_else(|_| body.to_string())` 兜底 |
| `src/main.rs` | endpoints HashMap 注册 `CliEndpoint` |
| `Cargo.toml` | 若缺则补 `chrono`（CLI endpoint 用） |

**不做的事**（避免阶段 2 蔓延）：
- ❌ 不改 Config 为 `Arc<RwLock<Config>>`（阶段 5）
- ❌ 不改 `cred.effective_endpoint_name()` 调用方式（保留 upstream 现有写法）
- ❌ 不动 provider.rs 除两处 caller 之外的任何逻辑
- ❌ 不接入 `usage_request_parts` 的 caller（阶段 4）

**验证**：
- `cargo check` / `cargo test` 通过（新增测试不减少 baseline 通过数）
- IDE 凭据走 IDE 端点行为与合并前一致（特别是 builder-id/idc 凭据请求体不含 profileArn）
- 准备一份 `endpoint: "cli"` 凭据，走 CLI 协议返回 200
- Tool 序列化输出验证：JSON 字段顺序应为 `inputSchema → name → description`

**预估**：1.5 天。

### 阶段 3：Anthropic 数据面（cache + 压缩 + 图片）

**目标**：移植 feature 在 Anthropic 兼容层的核心创新。这是整个方案最有价值的一阶段，也是冲突最复杂的之一。

**模块文件移植**：

| 文件 | 行数 |
|---|---|
| `src/image.rs` | 564 |
| `src/anthropic/compressor.rs` | ~1500 |
| `src/anthropic/cache_tracker.rs` | 940 |

**在 upstream 已有文件里加挂载点**：

| upstream 文件 | 需要加入的 hook |
|---|---|
| `src/anthropic/converter.rs` | 图片处理、压缩前预处理 |
| `src/anthropic/handlers.rs` | 压缩管道触发点、cache_tracker 记录点 |
| `src/anthropic/middleware.rs` | cache_tracker 请求/响应拦截 |
| `src/anthropic/stream.rs` | cache 计费拆分（保留 upstream 的 thinking 提取） |
| `src/anthropic/websearch.rs` | cache 在 fallback 时保留 |
| `src/model/config.rs` | 新增 `cache.*` / `compression.*` 配置项 |

**原则**：
- **不要**整文件覆盖——会丢掉 upstream 的 thinking 提取、API Key 支持
- **应该**手动 review feature 版本 → 在 upstream 文件里精确插入调用点
- 配置项**默认全部关闭**，确保对现有行为零影响

**验证**：
- 默认关闭情况下，跑通现有 e2e 用例
- 打开 cache_tracker，对比一轮对话的 token 计费拆分
- 打开图片压缩，跑一张 GIF + 大图请求

**预估**：3-5 天。

### 阶段 4：凭据栈增强（最高冲突）

**目标**：把 feature 的"凭据故障转移/限流/亲和性"接入 upstream 凭据管理。

**文件移植**：

| 文件 | 行数 |
|---|---|
| `src/kiro/rate_limiter.rs` | 484 |
| `src/kiro/cooldown.rs` | 388 |
| `src/kiro/affinity.rs` | 86 |
| `src/kiro/background_refresh.rs` | 346 |
| `src/kiro/fingerprint.rs` | — |
| `src/kiro/model/events/metering.rs` | — |
| `src/kiro/model/events/reasoning.rs` | — |

**关键挂载点**：

| upstream 文件 | 改动方式 |
|---|---|
| `src/kiro/provider.rs` ⚠️ | 保留 upstream 的 API Key 分支 + profileArn + endpoint trait；插入 rate_limiter 查询、cooldown 跳过、响应后回填 |
| `src/kiro/token_manager.rs` ⚠️ | 保留 upstream 的 invalid_grant 立即禁用、profileArn 同步；接入 affinity、background_refresh |
| `src/kiro/mod.rs` | 启动时 spawn `background_refresh::start(...)` |
| `src/main.rs` | rate_limiter / cooldown 注入到 KiroProvider 构造 |

**行为差异决策**：

| 场景 | 采用 | 理由 |
|---|---|---|
| 收到 429 | **feature** 模式 | cooldown 退避更细粒度 |
| `invalid_grant` | **upstream** 模式 | 立即禁用更安全 |
| profileArn | **upstream** 模式 | 动态注入正确性更高 |
| 启动余额查询 | **feature** 顺序模式 | 避免触发 429 |

**预估**：5-7 天。**这是整个方案的最大风险点**。

### 阶段 5：Admin 热加载

**目标**：`Config` 改 `Arc<RwLock<Config>>`，Admin UI 增加运行时配置切换。

**地基改造**：

| 文件 | 改动 |
|---|---|
| `src/main.rs` | `let config = Arc::new(RwLock::new(config));` |
| 所有读 config 的位置 | 改为 `config.read().xxx` |
| `src/admin/handlers.rs` | 增加 PATCH 端点支持 region/RPM/proxy/compression/cache 热更新 |

**Admin UI 前端**：合并 feature 的批量导入、热加载面板、凭据排序、defaultEndpoint 选择；保留 upstream 的 API Key 凭据流程。

**预估**：3-5 天。

### 阶段 6：诊断工具与 Web Portal

| 内容 | 操作 |
|---|---|
| `tools/*.py`、`.mjs` 诊断脚本 | 直接复制 |
| `src/kiro/web_portal.rs` | 复制 + `kiro/mod.rs` 挂载 |
| `sensitive-logs` feature flag | `Cargo.toml` 增加 feature，logging 出口处用 `#[cfg]` 控制 |

**预估**：1-2 天。

---

## 八、阶段汇总

| 阶段 | 内容 | 难度 | 工时 | 与后续 upstream 同步的冲突点 |
|---|---|---|---|---|
| 1 | 工具模块 | 低 | 0.5 d | 无 |
| 2 | CLI endpoint + trait | 低-中 | 1 d | trait 签名变化时 |
| 3 | cache + 压缩 + 图片 | 中-高 | 3-5 d | converter/handlers/stream 改动 |
| 4 | 凭据栈增强 | **高** | 5-7 d | provider/token_manager 改动 |
| 5 | Admin 热加载 | 中 | 3-5 d | Config 用法改动 |
| 6 | 诊断工具+Portal | 低 | 1-2 d | 无 |
| **总计** | | | **~3-4 周** | |

---

## 九、推荐实施次序

按依赖与价值的最优次序：**1 → 2 → 3 → 6 → 5 → 4**

理由：
- 阶段 4（凭据栈）放最后，因为冲突最复杂，前面阶段帮助熟悉 upstream 架构
- 阶段 6 放在 4 前面，因为诊断工具会大幅帮助阶段 4 的 debug
- 阶段 5（Config 改造）放在 4 前面：好处是 Config 改 Arc 后阶段 4 用得上；trade-off 是引入大面积 mechanical 改动

---

## 十、后续 upstream 同步策略

合并完成后，定期 `git fetch upstream && git merge upstream/master` 时：

- **阶段 1、6** 文件几乎不冲突
- **阶段 2** 冲突仅在 trait 签名变化时（罕见）
- **阶段 3** 是常态冲突源——upstream 改 anthropic 数据面时需手动 review 是否影响 cache/压缩 hook
- **阶段 4** 是高频冲突源——upstream 改 provider/token_manager 时需逐处确认 hook 仍生效
- **阶段 5** 中 Config 用法的冲突可批量解决（脚本化的 `.read()` 适配）

建议每次同步上游后，跑一遍 `cargo test` 和一组 e2e 用例（**阶段 3 完成时**就建立这套测试）。

---

## 十一、变更记录

| 日期 | 阶段 | 摘要 |
|---|---|---|
| 2026-05-19 | 0 | 撰写本文档 |
| 2026-05-19 | 1 | 移植 utf8 / redact / truncation / tool_compression |
| 2026-05-19 | 2 | 移植 CLI endpoint；升级 trait 签名为 `Result<String>`；新增 `UsageRequestParts`；`ToolSpecification` 字段顺序对齐 kiro-cli 2.3.0 wire |
| 2026-05-19 | 3.1 | 移植 `src/image.rs`；加 `CompressionConfig` 完整 schema（压缩字段为 3.2 预留）；converter.rs 当前消息图片块接入单图缩放；新增 `image` / `base64` 依赖 |
| 2026-05-19 | 3.2 | 移植 `src/anthropic/compressor.rs`（四层压缩管道）；AppState 加 `Arc<CompressionConfig>`；`convert_request` 签名加 `&CompressionConfig` 并在末尾接入 `compressor::compress`；`convert_tools` 后接入 `tool_compression::compress_tools_if_needed`；3.1 的图片 hook 改为接收 config（用户配置真正生效） |
| 2026-05-19 | 3.3 | 移植 `cache_tracker.rs` 与 `PromptCacheRuntime` 基础设施；types.rs 加 `CacheControl` 并扩展 SystemMessage/Tool 的 cache_control 字段；token.rs 加 3 个 count_* 计数函数；AppState 加 `Arc<RwLock<PromptCacheRuntime>>`；Config 加 `prompt_cache_ttl_seconds` / `prompt_cache_accounting_enabled`。**模块就位、caller 未接入**——stream/handlers/websearch 的 cache 拆分需重写 upstream 主体，留待用户主动决策 |
| 2026-05-19 | 6.1 | 移植 6 个诊断脚本到 `tools/`：analyze_compression、diagnose_improper_request、replay_demo_request、test_400_improperly_formed、test_empty_content、test_prompt_cache_usage。零 Rust 代码改动 |
| 2026-05-19 | 6.3 | 移植 `src/kiro/web_portal.rs`（554 行，Kiro Web Portal API 查询账户/订阅/用量）；`kiro/mod.rs` 加 `pub mod web_portal;`；Cargo.toml 新增 `serde_cbor 0.11`（rpc-v2-cbor 协议）。feature 自身也是 dead-code（文件级 allow），预留 future Admin UI 集成 |
| 2026-05-19 | 6.2 | Cargo.toml 加 `sensitive-logs = []` feature；main.rs 主凭证日志 / handlers.rs 两处 Kiro request body 日志加 `#[cfg(feature = "sensitive-logs")]` 守卫，默认输出摘要字段，启用后输出完整内容。两种 feature 配置都通过编译与测试 |
