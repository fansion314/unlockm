# Online Metadata Search with Free Primary Providers and Optional Spotify

## Status

In Review

Last updated: 2026-08-26

## Objective

在 UnlockM 桌面端现有的单曲元数据编辑弹窗中加入用户主动触发的联网搜索。默认/主要路径将使用不登录的 MusicBrainz 文本 metadata 搜索、MusicBrainz Cover Art Archive 专辑封面与 TheAudioDB fallback；Spotify 保留为 secondary、可选 provider，并明确提示其 Premium 账户要求。所有候选仍只填入草稿，最终只能由既有“保存”路径原子写入文件。

技术上保留可扩展的 provider 边界、受限 Rust-only 网络客户端与 provider-neutral IPC DTO。Spotify 的 PKCE/安全凭证实现保留给可选登录路径；免费 primary provider 不需要账户、OAuth 或 credential persistence。任何 token、原始 provider payload 或图片 URL 均不得进入 SQLite、webview 或日志。

## Scope

- 在 `desktop/src/features/metadata/MetadataEditor.tsx` 增加独立的“联网搜索”区域，覆盖未配置、未登录、登录中、已连接、需重新授权、搜索中、无结果、限流和失败状态。
- 只在用户点击登录、搜索或重试时联网；打开应用、打开编辑弹窗、编辑字段和保存本地标签本身不自动联网。
- 默认 provider 将为 MusicBrainz 文本 metadata、Cover Art Archive 主封面与 TheAudioDB fallback；Spotify 是清楚标注 Premium requirement 的 optional secondary provider。
- 候选展示标题、歌手列表、专辑、时长、Spotify 来源标识、可验证的 Spotify 跳转入口和由 Rust 懒加载的专辑封面预览；不依赖已在 2026 Development Mode 响应中移除的 `popularity`、`available_markets` 等字段。
- 用户可以分别填入文本、使用封面或填入全部。封面 bytes 只进入现有编辑草稿，不立即写文件、不释放播放器；用户可以继续修改或取消。
- 使用 Spotify Authorization Code with PKCE，不配置或保存 client secret。回调由 Rust 在 `127.0.0.1` 动态端口上的一次性 loopback listener 接收，并严格校验 state、callback path、请求大小和超时。
- 将 refresh token 和签发时间存入操作系统凭证库；access token 仅在内存缓存，并在过期前刷新。支持 Spotify 当前 6 个月 refresh-token 生命周期、提前提示重新授权、token rotation 和断开账号。
- Spotify client ID 作为公开构建配置注入；缺失时应用仍可构建和运行，但 provider 明确显示“未配置”且不能发起 OAuth。
- Rust 侧限制固定 HTTPS 主机、禁用重定向、限制响应体/查询长度/并发/超时，并正确处理 401、429 `Retry-After`、5xx、断网和畸形响应。
- Spotify API 返回的 image URL 永不进入 webview。Rust 仅把经过 Spotify CDN allowlist 验证的 URL 绑定到随机、不透明、短期内存的候选封面 handle；webview 只能以 handle 请求懒加载的已验证图片 bytes。handle 最多对应当前 10 个结果，在新搜索、Disconnect 或 TTL 到期时清除，不写 SQLite 或 catalog cache。
- 更新中/英/日文案、隐私/配置说明、安全测试、第三方依赖声明、CI/release 配置和 `ARCHITECTURE.md`。
- 将桌面元数据编辑器改为本地草稿/联网搜索左右工作区；结果以有字段标签的候选列表呈现，窄窗口回退为单栏，左右内容保持各自可滚动且底部 Save actions 固定可达。
- 在线封面获取成功后必须在本地草稿区立即显示预览和来源/未保存状态；失败或无封面时在搜索控件附近显示可见、可恢复的提示，不能把反馈放在长结果列表末尾。
- Cover Art Archive 只接受由 release-group MBID 构造的固定 HTTPS endpoint；其 Internet Archive 存储重定向最多 3 次且仅允许 `archive.org`/子域。TheAudioDB 正确处理真实 `album:null`、使用原始 artist/album 查询并把规范化仅用于结果比较；两者均使用 10s connect/20s total timeout、bounded body 和现有图片验证。

## Non-Goals

- 本计划不把 Spotify 搜索接入 CLI、批量转换、watch mode 或 `--update-metadata`；现有 QQ/NCM 自动补全路径保持不变。
- 不做自动匹配、自动保存、批量补全或后台预取；用户必须明确选择候选并再次点击现有保存操作。
- 不下载 Spotify audio preview 或音频内容；不长期缓存、展示为独立 catalog，或自动导入封面。专辑封面仅可由用户逐条明确选择，作为现有 metadata 编辑草稿的一部分经过既有 Save 写入。
- 不把 Spotify catalog 搜索结果长期写入 SQLite，不建立离线 catalog cache，不记录搜索历史或用户 Spotify profile。
- 不请求 Spotify 播放、媒体库、邮箱或私密资料 scopes；初版只申请搜索所需的最小授权。
- 在官方 API 约束完成核对前，不承诺 MusicBrainz/TheAudioDB 的聚合排序、缓存、速率策略或 provider fallback；Spotify 多账号也不在范围内。
- 不向 webview 开放任意 URL、通用 HTTP、shell、process、filesystem、asset protocol 或 opener 权限。
- 不在本计划中申请 Spotify Extended Quota、创建开发者账号或代表项目接受平台条款；这些是发布前的外部账户/合规门禁。

## Current Context

- `desktop/src/features/metadata/MetadataEditor.tsx` 已有 title/artist/album 草稿、本地历史封面候选、10 MiB 上传、focus trap、viewport-bounded body 和固定 actions；`save()` 才会释放当前播放器并调用 `updateRunItemMetadata`。
- `desktop/src/features/metadata/MetadataEditor.test.tsx` 当前覆盖本地封面、保存/失败时播放器释放恢复、弹窗布局和上传封面回写，可直接扩展联网搜索的“只填草稿、不提前保存”回归。
- `desktop/src/lib/contracts.ts` 与 `desktop/src/lib/ipc.ts` 只定义本地 metadata patch/artwork IPC；尚无 provider、登录状态、搜索候选或限流 DTO。
- `desktop/src-tauri/src/commands/mod.rs` 的 `update_run_item_metadata` 已使用 ID-only output lookup、`IdleHistoryMutationGuard`、同目录候选、pending marker、回读 reconciliation 和原子发布。在线搜索不能绕过或复制这条写入路径。
- `desktop/src-tauri/src/state.rs` 只拥有 conversion/history mutation 状态。OAuth/token/search 应放入单独的 Tauri-managed `OnlineMetadataState`，避免把外部账号生命周期耦合到 conversion active slot。
- `desktop/src-tauri/src/lib.rs` 已有 Rust-only opener 调用和严格 invoke registration；webview 没有 opener permission。Spotify 授权页与结果页也应由 Rust 根据固定 provider/ID 构造并打开。
- `desktop/src-tauri/src/history/mod.rs` 的 SQLite schema v3 保存本地 history/preferences/artwork。凭证不属于 history，新增 Spotify 登录不需要 schema migration。
- `src/metadata/provider/mod.rs` 目前只有 QQ provider；`src/metadata/model.rs` 的 `Metadata`/`Candidate` 面向自动合并。手工在线搜索需要独立的 provider-neutral search DTO，不能把用户明确选择误装成自动 `verified_search()`。
- 根 crate 已使用 bounded `reqwest`/Rustls，现有 QQ client 已有 no-redirect、timeout、body cap、host allowlist 和 test transport 模式，可复用安全模式但不能复用 QQ host/response 模型。
- `desktop/src-tauri/tauri.conf.json` 的 CSP 禁止 webview 直接连接外网或加载远程图片；`desktop/src-tauri/tests/security_policy.rs` 把这一点作为门禁。本计划保持 CSP 不变，所有 Spotify HTTP 仅由 Rust 发起。
- `docs/plans/2026-08-23-desktop-history-player-metadata-ux.md` 仍为 `In Review`，其中“外部网络封面/新 metadata 服务”为当时范围外项。本计划是基于已经实现的编辑器安全边界开展的新增量，不修改旧计划状态，也不重开其 Windows QA 缺口。
- Spotify 官方当前建议不能安全保存 client secret 的桌面/移动客户端使用 [Authorization Code with PKCE](https://developer.spotify.com/documentation/web-api/tutorials/code-pkce-flow)。
- Spotify [Redirect URI](https://developer.spotify.com/documentation/web-api/concepts/redirect_uri) 要求 loopback 使用显式 `127.0.0.1` 或 `[::1]`，不允许 `localhost`；loopback 注册 URI 可省略端口并在授权请求中使用动态端口。
- Spotify [Refreshing tokens](https://developer.spotify.com/documentation/web-api/tutorials/refreshing-tokens) 文档当前说明 access token 约 1 小时，Developer Dashboard app 的 refresh token 生命周期为 6 个月，PKCE refresh 需要 client ID 且不需要 client secret。
- Spotify [Search](https://developer.spotify.com/documentation/web-api/reference/search) 与 [February 2026 migration guide](https://developer.spotify.com/documentation/web-api/tutorials/february-2026-migration-guide) 将 Development Mode search `limit` 上限降为 10，并移除部分响应字段；Development Mode app owner 需 Premium，且新 app 默认最多 5 个用户。公共发布需确认 Extended Quota 或可接受的用户范围。
- Spotify [Rate Limits](https://developer.spotify.com/documentation/web-api/concepts/rate-limits) 使用滚动 30 秒窗口，429 通常提供 `Retry-After`；UI 和 adapter 必须暴露可理解的冷却状态而非盲目重试。
- Spotify [Developer Policy](https://developer.spotify.com/policy) 要求展示 Spotify 内容时提供来源标识与对应内容链接，并限制将 metadata/cover art 作为独立服务。首版仅允许用户逐条导入封面到现有编辑草稿、不缓存 catalog，并把项目所有者的条款确认列为启用 production adapter 的 release gate。

## Design and Decisions

### 0. Free-primary revision (official constraints confirmed)

- MusicBrainz root is `https://musicbrainz.org/ws/2/`; non-commercial use is free, no API key is needed, a meaningful User-Agent is mandatory, and each client starts at most one request per second. Use `UnlockM/<pkg version> (https://git.fansionia.world/fansion/unlockm)` with an injectable scheduler. Source: [MusicBrainz API](https://musicbrainz.org/doc/MusicBrainz_API).
- Search recordings with a bounded plain-text `dismax=true` query (MusicBrainz escapes special syntax and searches default recording-index fields) and limit≤10; consume title, artist-credit, length, releases and release-group, flatten/dedupe recording+release-group.
- Cover Art Archive release-group `front-500` is the primary artwork source. Exact API host is `coverartarchive.org`; storage redirects are HTTPS-only, at most 3, and limited to `archive.org` or subdomains. TheAudioDB public key `123` is non-secret and remains fallback: album-mb first, then raw artist+album search with unique normalized exact response comparison. Exact TADB hosts remain API `www.theaudiodb.com` and image `r2.theaudiodb.com`; artwork is on-demand only.
- Free primary search has no auth status or credential persistence. Spotify remains optional secondary and its Premium live path does not block M6–M8.

### 1. Provider boundary and search semantics

- 在 `src/metadata/provider/` 增加 provider-neutral `OnlineMetadataCandidate`、`OnlineMetadataError` 和最小 `OnlineMetadataProvider` 接口；在 `spotify.rs` 封装 Spotify authorize/token/search 协议、响应模型和固定主机策略。
- 候选包含 `provider`、opaque provider item ID、title、`artists: Vec<String>`、album、duration、可选 external link capability；不把 token、原始 response、图片 URL 或 provider payload 传给 webview。
- 前端应用候选时按统一的 UI display convention 把 artists 数组连接为一个可编辑字符串；后端保存仍执行现有 512-character trim/clear 校验。
- 搜索 query 由弹窗当前 title/artist/album 初始生成，但只在用户主动提交时发送。query 最多 200 个 Unicode scalar values；结果 limit 在 Rust 强制 clamp 到 1–10。
- provider 结果是用户选择建议，不进入现有自动 merge/provenance 优先级，也不直接调用 tag writer。点击结果只更新 React state；现有 Save 是唯一文件 mutation。
- 首版不缓存 catalog 结果。为防止迟到响应覆盖新查询，前端使用 request generation 丢弃 stale result；Rust 只保留 token 状态，不保留搜索历史。
- Spotify search response 中的 album image URL 只在 Rust adapter 内解析和验证，映射为 128-bit random opaque `cover_handle`。handle 表有 ≤10 项和短 TTL；候选文本响应不携带 URL。前端以 handle 请求预览 bytes，Rust 再次查表、执行 fixed-host/no-redirect/bounded fetch，并调用现有 `validate_cover`（MAX_COVER_BYTES、magic/MIME、decodability）后才返回。
- “使用封面”或“填入全部”只把 Rust 返回的 validated image bytes 设置为现有 React `cover` draft；对象 URL 在替换、关闭和 stale response 时 revoke。单独“填入文本”仍不改变本地封面草稿。唯一落盘点仍为既有 Save。

### 2. Spotify PKCE login and loopback callback

- `OnlineMetadataState` 同一时间只允许一个 Spotify auth flow。启动时先绑定 `127.0.0.1:0`，再生成 32-byte 以上 cryptographically secure verifier/state，计算 S256 challenge，并让 Rust opener 打开固定 `https://accounts.spotify.com/authorize` URL。
- Developer Dashboard 注册 `http://127.0.0.1/callback`（无端口）；实际请求使用已绑定的动态端口。不得使用 `localhost`、自定义 URI scheme 或前端 dev server 作为 callback。
- callback listener 只接受 loopback、固定 `/callback` GET、有限长度 request line/query；state 必须 constant-time match。拒绝 wrong state/path、重复 callback 和畸形 percent encoding。
- authorization code、verifier 和 state 仅存于 flow memory。成功、拒绝、取消或 3 分钟超时后都清除；listener 返回不含 token/code 的静态成功/失败 HTML，再结束 flow。
- 前端 `startOnlineMetadataLogin("spotify")` 等待 async command 完成并显示进度；另有 `cancelOnlineMetadataLogin` 触发 Rust cancellation。重复点击返回稳定 busy 状态，不启动第二个 listener/browser。
- 不请求用户 profile；connection status 只显示 configured/connected/reauthorizationRequired/credentialUnavailable，不显示用户名或邮箱。

### 3. Credential and token lifecycle

- 在 `desktop/src-tauri/src/online_metadata/credential_store.rs` 定义可 mock 的 `CredentialStore`，production 实现使用最新稳定 `keyring` crate 映射 macOS Keychain 与 Windows Credential Manager。
- service/account key 使用稳定 app identifier + provider + single-account slot。保存的 secret payload 只含 refresh token、issued-at、client-ID fingerprint 和格式版本；client ID 本身是公开配置，不是 secret。
- 不提供 SQLite、JSON 文件、localStorage 或环境变量 token fallback。安全凭证库不可用时返回 `credential_unavailable`，不能静默降级为明文持久化。
- access token 和 expires-at 只保存在进程内，提前 60 秒视为过期；并发搜索通过 single-flight refresh 避免 refresh stampede。刷新响应未返回新 refresh token 时保留旧值，返回 rotation token 时先安全写入再切换内存状态。
- issued-at 距 6 个月不足 14 天时状态提示重新授权；服务端 `invalid_grant`/持续 401 也进入 reauthorizationRequired。普通断网/5xx 不删除仍可能有效的 refresh token。
- Disconnect 清理内存 token、取消 auth/search、删除系统凭证并清除 provider session 数据。Spotify 没有由本计划假定的 revoke endpoint；文档说明“断开”只删除本地凭证，如需撤销授权应使用 Spotify 账号的 Apps 页面。

### 4. Public client configuration and release gating

- client ID 通过 compile-time `UNLOCKM_SPOTIFY_CLIENT_ID` 注入，`desktop/src-tauri/build.rs` 声明 `rerun-if-env-changed`。不引入 client secret，也不把 client ID 误标为 secret。
- 无 client ID 的本地/CI build 必须成功，status 返回 notConfigured；provider tests 使用构造函数注入 fixture client ID，不依赖真实账号或网络。
- 开发/发布文档记录 Spotify app 创建、redirect URI、Development Mode Premium/5-user 约束和 6-month reauthorization。面向超过 allowlist 的公开构建在 Extended Quota/平台条款未确认前不得宣称 Spotify 功能普遍可用。
- `.github/workflows/ci.yml` 保持无真实 Spotify 网络的确定性测试。启用发布 adapter 时，`desktop-release.yml` 从 repository variable 注入公开 client ID，并在打包前检查“非空且未出现 client secret”；不改变现有 no-signing-secret release contract。

### 5. Network, URL, and error safety

- token/search clients 只允许 `https://accounts.spotify.com/api/token` 与 `https://api.spotify.com/v1/search`，禁用 redirects、userinfo、IP literal、非默认 scheme/host 和调用方提供的 base URL；测试 transport 才可指向 loopback fixture。
- 固定 connect/total timeout、最大 response body 和 bounded serde parsing。错误体只映射为稳定 code，不把 response body、query、authorization code、verifier 或 token写入日志。
- 首次 401 允许 single-flight refresh 后重放一次搜索；第二次 401 进入 reauthorizationRequired。429 解析并 clamp `Retry-After`，返回 retryAt；UI 在冷却结束前禁用重复提交。5xx/timeout/offline 保持草稿和凭证不变。
- “在 Spotify 中打开”只把 provider + validated Spotify track ID 交给 Rust，由 Rust 构造 `https://open.spotify.com/track/{id}`；webview 不能传任意 URL。OAuth authorize URL也只由 Rust 构造。
- 不为搜索放宽 webview CSP 的 `connect-src`/`img-src`，不新增 opener frontend permission。所有外部导航必须经过固定 provider adapter。

### 6. Metadata editor UX

- 在现有文本字段与封面区域之间加入可折叠 online section。初始 query 由当前草稿生成，用户可编辑；Enter/按钮搜索，Escape 仍关闭整个 dialog，Tab 顺序继续受现有 focus trap 管理。
- 桌面宽度下 dialog 使用左右工作区：左侧编辑 title/artist/album、当前封面草稿和本地封面候选；右侧选择 provider、搜索和浏览联网候选。结果列表按标题、歌手、专辑、时长、元数据来源、封面来源显示明确字段标签；不使用无语义的点分隔长文本。
- modal actions 独立于两个滚动区；小于 860px 时显式回退单列并恢复整体纵向滚动。所有 CTA 在桌面保持单行，深浅主题继续使用现有 semantic tokens。
- 未登录显示 Connect Spotify；登录中显示 cancel；已连接显示搜索框、Search 和 Disconnect；即将/已经过期显示 Reconnect。notConfigured 与 credentialUnavailable 给出可操作但不泄露内部路径的说明。
- 每条 result 是键盘可达的 Apply button/row，展示 title、artists、album、duration 和 Spotify attribution，并提供独立“Open in Spotify”操作；两者点击不能触发 Save 或关闭 dialog。
- Apply 只替换 title/artist/album state，保留 cover/upload/local candidate 和 search results，设置可见的“已填入，尚未保存”反馈。取消 dialog 丢弃应用结果；保存继续调用唯一 `updateRunItemMetadata`。
- Use cover/Fill all 获取到 bytes 后创建仅供当前草稿显示的 object URL，并在左侧封面区显示预览及成功状态；替换、切换本地封面或卸载时 revoke。lookup/fetch 失败在搜索头部和对应候选处提示，草稿与已有封面保持不变。
- 搜索错误与文件保存错误使用不同 state/alert，Retry 语义不混用；迟到搜索不能覆盖更新后的 query/result，登录或断开不会清空用户已编辑的 metadata fields。

### 7. Compatibility, policy, and documentation

- Spotify response structs 对 February 2026 removed fields 全部 optional/ignored，不依赖 popularity 排序；使用 API 返回顺序并限制 10 条。
- Spotify 结果区域始终包含来源标识和可访问的 track link。专辑封面只由用户逐条导入，使用 Rust-only temporary handle、lazy fetch 和 existing cover validation，不长期缓存，也不把 Spotify 做成单独 catalog 页面。
- 在 `README.md`/`README.zh.md` 说明显式联网触发、存储内容、断开方式、6 个月重新授权、Spotify 配置/配额和无 telemetry；三语言应用文案保持 parity。
- 完成后更新 `ARCHITECTURE.md` 的 provider extension、OAuth/credential state、Rust-only network/opener、no-network-on-open 和 secret-storage invariants。

## Milestones

> 2026-08-25 priority revision: M1–M4 record completed/reviewed Spotify-secondary foundation work. Before further implementation, define new primary-provider milestones for MusicBrainz text search and TheAudioDB artwork after official API constraints are verified. M5 native QA must test the free primary path first; Spotify QA becomes optional Premium-account coverage.

### M11 — Reliable free artwork sources

- Goal: 让 MusicBrainz 候选优先从 Cover Art Archive 获取封面，并修复 TheAudioDB 真实 null/timeout/query fallback，使“未收录”和网络失败不再互相伪装。
- Dependencies: M6 free provider boundary, M9 connection evidence, M10 visible cover feedback.
- Acceptance: TENT release-group 通过 CAA 得到 validated cover；CAA 404/timeout 安全 fallback TADB；TADB `album:null` 视为空、原始多词 query 可命中、10s/20s bounds 生效；fixed-host/manual redirect/body/image/generation invariants 不变。
- Validation: local redirect/null/raw-query/timeout/oversize/malformed fixtures，security semantic guards，real CAA TENT and TADB Parachutes/RAM evidence，native cover draft Apply/Cancel。

### M12 — v0.4.0 release

- Goal: 将 M9-M11 与完整多色主题集成作为 v0.4.0 可复现发布状态提交并同步两个远端。
- Dependencies: M11 implementation and final review pass.
- Acceptance: root/desktop package versions 0.4.0、lock/config/docs 一致；full automation and debug bundle pass；related files committed；annotated `v0.4.0` tag points to reviewed commit；`main` and tag pushed to `origin` and `github` without force.
- Validation: version audit、full plan validation matrix、commit/tag object checks、remote branch/tag verification。

### M9 — User-acceptance regression fixes

- Goal: 修复原生编辑器中标题输入框持续抢回焦点，以及 MusicBrainz 在 TLS 建连完成前被 3 秒 connect timeout 截断的问题。
- Dependencies: M6–M7 implemented provider/editor paths; user-provided native reproduction.
- Acceptance: 初始标题聚焦只发生一次，父组件使用新 callback identity 重渲染后其他控件仍保有焦点；MusicBrainz DNS/TCP/TLS 建连有 10 秒上限、完整请求仍受 20 秒总上限约束；title+artist 自由文本输入使用 MusicBrainz dismax 跨字段语义并返回相关候选；fixed-host/no-redirect/1rps/body cap 不变。
- Validation: frontend rerender/focus regression test、MusicBrainz timeout/query constants and unit fixtures、exact production-client probe、native debug UI focus/search checks、targeted/full automated checks。

### M10 — Desktop metadata workspace and visible artwork draft

- Goal: 用适合桌面的双栏工作区替换单列长弹窗，并让候选字段和在线封面应用结果清晰可见。
- Dependencies: M7 provider-neutral editor UX and M9 reliable/relevant MusicBrainz search.
- Acceptance: ≥860px 左编辑/右搜索且各自滚动，窄屏单列；候选以字段化列表显示且按钮不折行；online cover 成功即时显示左侧预览/来源/未保存状态，失败/无封面有可见提示；Apply 仍只改草稿，Save 边界不变。
- Validation: DOM/layout/locale/cover object-URL success/error/stale cleanup tests，full Vite+ fmt/check/test/build，debug `.app` 双主题与原生宽/窄窗口 QA。

### M6 — Free primary provider core

- Goal: MusicBrainz recording text search is the no-login default; TheAudioDB supplies cover art only on explicit user demand.
- Dependencies: existing bounded Rust client and opaque cover-handle boundary.
- Acceptance: exact MusicBrainz root/UA/1rps scheduler/limit10/flatten-dedupe; TheAudioDB public `123` album-mb then unique exact fallback; fixed hosts/no redirect/validated image; no automatic cover lookup.
- Validation: deterministic local UA/rate/query/response/image/race fixtures.

### M7 — Free-first provider UX and security

- Goal: MusicBrainz Free/no-login is default; Spotify is optional Premium-required secondary.
- Dependencies: M6.
- Acceptance: provider switch invalidates results/handles; MB needs no Connect; attribution/open and on-demand cover are visible; CSP unchanged.
- Validation: frontend default/switch/stale/attribution/Use-all tests, locale and semantic security gates.

### M8 — Free-primary integration and QA

- Goal: validate the no-account primary path on macOS and retain Windows/Spotify-live gaps truthfully.
- Dependencies: M6–M7.
- Acceptance: MusicBrainz/TheAudioDB macOS primary QA; commercial distribution remains gated on MetaBrainz plan confirmation.
- Validation: full automation plus native no-account matrix.

### M1 — Provider foundation, PKCE, and secure credentials

- Goal: 建立无 client secret 的 Spotify 登录和可跨重启恢复的系统凭证状态。
- Dependencies: 项目所有者提供可用于开发 QA 的 Spotify client ID 与 allowlisted account；production enablement 另受配额/政策门禁约束。
- Acceptance criteria:
  - PKCE S256、loopback dynamic port、state validation、timeout/cancel 和 one-active-flow 均由 Rust 拥有并有确定性测试。
  - refresh token 只进入 OS credential store；SQLite/webview/log/test snapshot 不含 secret。
  - restart 后可从 refresh token 恢复连接；rotation、6-month reauth、disconnect 和 keychain failure 语义明确。
  - 无 client ID 时 app 正常启动且 provider 为 notConfigured。
- Validation:
  - provider/auth/credential unit tests 使用 mock clock、mock opener、mock credential store 和 local HTTP fixture。
  - `rtk cargo test --locked -p unlockm-desktop online_metadata -- --test-threads=1`。
  - 手工确认 macOS Keychain/Windows Credential Manager 创建、重启读取和 Disconnect 删除。

### M2 — Bounded Spotify search adapter

- Goal: Rust 可用有效 token 安全搜索 Spotify track 并映射成稳定的 provider-neutral candidates。
- Dependencies: M1 token lifecycle。
- Acceptance criteria:
  - 手动 query 返回至多 10 条 title/artists/album/duration/ID 候选，缺失 2026 removed fields 不报错。
  - 固定 host、no redirect、query/body/time/concurrency bounds 生效；401 refresh-once、429 Retry-After、5xx/offline 有稳定错误码。
  - 不传递原始 image URL/token/response body 到 webview，不产生 catalog persistence；每个候选有临时 opaque cover handle，Rust-only lazy fetch 通过 CDN allowlist、no redirect、body/image validation。
  - provider result opener 只能打开由验证过的 Spotify track ID 构造的固定 URL。
- Validation:
  - root provider unit/integration tests 覆盖正常、空结果、malformed JSON、oversize、redirect、host spoof、401/429/5xx 和 refresh race。
  - `rtk cargo test --locked --all-targets --all-features metadata::provider::spotify`。

### M3 — Metadata editor search and quick-fill UX

- Goal: 用户能在现有元数据弹窗中登录、搜索、比较并把选中结果安全填入草稿。
- Dependencies: M1–M2 contracts/commands。
- Acceptance criteria:
  - 所有 connection/search/error states、三语言文本、键盘/focus/reduced-motion 和矮窗口滚动均可用。
  - 打开 dialog 不联网；只有明确登录/搜索/重试触发网络。
  - Text/cover/all Apply 都只更新草稿，不调用 update IPC、不写文件、不释放播放器；文本 Apply 不替换封面，cover/all Apply 仅设置现有 cover draft。
  - Save 仍只调用现有 `updateRunItemMetadata` 并保持成功/失败播放器恢复语义。
  - Spotify attribution/link 与结果/封面预览同屏，Disconnect 易于访问；所有 Apply 操作都不触发 Save。
- Validation:
  - `MetadataEditor.test.tsx` 覆盖各状态、stale response、Apply-vs-Save、disconnect、rate-limit、reauth、focus trap 和 viewport contract。
  - `rtk vp -C desktop fmt . --check`、`rtk vp -C desktop check --no-fmt`、`rtk vp -C desktop test`、`rtk vp -C desktop build`。

### M4 — Security, privacy, configuration, and documentation

- Goal: 新外部账号/网络能力维持现有最小权限与可发布说明。
- Dependencies: M1–M3。
- Acceptance criteria:
  - CSP/ACL 不增加外网 connect/img、generic opener、shell/process/fs/asset 权限。
  - security tests 静态确认无 client secret、无 token persistence fallback、固定 provider hosts 和 ID-only result opener。
  - README/architecture/三语言文案说明联网时机、凭证位置、断开/重授权、client configuration、配额与政策门禁。
  - 新 Rust dependencies 使用最新稳定版，license/notice 与 locked builds 更新完整。
- Validation:
  - `rtk cargo test --locked -p unlockm-desktop --test security_policy`。
  - `rtk cargo test --locked -p unlockm-desktop --test license_audit`。
  - workflow/config static assertions、locale parity 和 `rtk git diff --check`。

### M5 — Cross-platform integration and native QA

- Goal: 在真实 macOS/Windows Tauri app 中证明 OAuth、secure persistence、search、Apply 和原子 Save 可用。
- Dependencies: M1–M4；测试 Spotify app/account 可用。
- Acceptance criteria:
  - 两平台完成 login → search → apply → save MP3/FLAC → restart → refresh/search → disconnect 流程。
  - denied consent、wrong state fixture、timeout、offline、429、expired refresh token 和 keychain locked/unavailable 有可理解恢复路径。
  - SQLite、应用日志、webview devtools/IPC payload 检查均无 token/code/verifier；Disconnect 后 OS credential entry 消失。
  - 未取得真实 Windows/macOS/Spotify 外部证据时计划保留 `In Review` 并明确缺口，不以 mock 或另一平台替代。
- Validation:
  - 执行完整 `Validation Strategy`。
  - macOS/Windows configured no-bundle/package build 和 native manual matrix。
  - 独立 review 检查安全、政策、计划状态和验证证据。

## Task Breakdown

- [x] T11.1 新建 `src/metadata/provider/coverartarchive.rs`：release-group `front-500` lookup、10s/20s bounded clients、no automatic redirects for lookup、最多 3 次且仅 `archive.org`/子域的 HTTPS fetch redirects、body cap/`validate_cover` 和恶意 redirect fixtures。
- [x] T11.2 修复 `theaudiodb.rs`：10s connect/20s total；真实 `album:null` 映射为空并继续 fallback；API 发送 trim 后原始 artist/album，normalize 只用于唯一 exact response comparison；补 raw-vs-normalized 与 null fixtures。
- [x] T11.3 在 desktop free service/state 接入 CAA primary/TADB fallback 与 host-dispatched fetch，更新 provider capability/候选 attribution、三语 UI、README/architecture/security guards；保留 opaque handle/single-flight/generation/draft-only Save。
- [x] T11.4 执行 provider/desktop/frontend/security 全量自动化及 macOS native TENT cover Apply/Cancel QA；独立 review 所有 fixed-host/redirect/fallback/error/plan evidence，解决全部 findings。
- [x] T12.1 将 root/desktop/Tauri 版本与 lockfile 更新为 0.4.0，核对 release docs/config/notice，无新增依赖或 secret。
- [x] T12.2 执行完整 release validation、debug no-bundle/app bundle 和最终 plan review，记录 warnings、commit candidate 与 remaining external gaps。
- [ ] T12.3 conventional commit 全部相关变更，创建 annotated `v0.4.0`，推送 `main` 与 tag 到 `origin`、`github` 并读取远端 refs 验证。

- [x] T10.1 重构 `MetadataEditor.tsx` DOM 和 `history.css`：桌面 46/54 双栏、独立滚动、固定 actions、左侧字段/封面草稿、右侧搜索；<860px 显式单栏回退并保持 focus trap。
- [x] T10.2 将 MusicBrainz/Spotify candidate 统一渲染为字段化结果列表，加入标题/歌手/专辑/时长/元数据来源/封面来源标签、结果计数、loading/empty/error 头部状态和不折行 actions。
- [x] T10.3 修复 online cover 草稿可见性：成功创建/revoke object URL、显示左侧预览和来源反馈；失败/无封面显示候选级及头部 alert；上传/本地候选替换时清理不再生效的在线预览。
- [x] T10.4 更新中英日文案与 `MetadataEditor.test.tsx`，覆盖 split layout、字段标签、结果计数、cover success/error/stale cleanup/draft-only Save；完成全量 Vite+ 和原生 debug `.app` 宽屏浅/深主题 QA。窄屏由 860px CSS source contract 覆盖；Computer Use 无可靠窗口 resize，作为已记录工具限制而非原生通过。
- [x] T10.5 修复本地封面候选 grid 的 intrinsic image overflow：稳定三列/固定图片行、卡片 overflow containment、两行专辑名和滚动高度；增加多候选 CSS source contract 与原生视觉复验。

- [x] T9.1 拆分 `MetadataEditor.tsx` 的一次性初始聚焦与 Escape/Tab listener lifecycle；新增父组件以新 `onClose` identity 重渲染时不会抢回标题焦点的回归测试。
- [x] T9.2 将 MusicBrainz production connect timeout 从 3 秒修正为 10 秒，同时保留 20 秒 total timeout；新增常量断言并用仓库真实 client 复测 TLS 建连不再在约 3 秒提前失败。
- [x] T9.3 更新计划/架构中的网络与焦点约束，执行定向前端、provider、desktop check 和 diff validation，记录真实端点仍可能返回的服务端状态而不误判为客户端超时。
- [x] T9.4 修复 MusicBrainz `recording:("title artist")` 与 `dismax=true` 组合导致结果退化为通用 “Recording” 的 live relevance finding；改用 bounded plain-text dismax query，更新 request fixture，并在原生 debug UI 证明 `relive toconoma` 返回相关候选。

- [x] T6.1 在 `src/metadata/provider/musicbrainz.rs` 定义固定 root、meaningful UA、可注入全局 1rps scheduler、bounded/no-redirect client 和 recording search request；用 local fixture 验证 UA、rate scheduler、host/redirect/oversize/503/timeout/offline。（M6 re-review fixes：dismax、production scheduler semantics、root spoof/slow timeout。）
- [x] T6.2 实现 MusicBrainz query/hint bound、plain-text dismax/provider-side syntax escaping、recording JSON mapping、artist-credit/release/release-group flatten 与 recording+release-group dedupe；保留 MB recording/release-group IDs，limit clamp10。（M6 re-review fixes：DTO artist clean names，Rust-only credited cover hint；M9 review 修正跨字段 relevance。）
- [x] T6.3 在 `src/metadata/provider/theaudiodb.rs` 实现公开 key `123` 的固定 endpoint、album-mb 优先与唯一 normalized artist+album `searchalbum` fallback；API/image exact host allowlist、no redirect、body/image cap 和 `validate_cover`。
- [x] T6.4 扩展 online state 的 cover binding 为 `MusicBrainz { release_group_id, artist, album }`；仅 Use cover/Fill all 懒触发 TheAudioDB lookup+fetch，最多一 lookup、复用 image permits、TTL/single-flight/generation/disconnect/new-search cleanup。
- [x] T6.5 扩展 provider-neutral Rust DTO/error/capability 与 commands：MusicBrainz 默认 status requiresAuthorization=false、fixed recording opener、cover attribution；Spotify 保持 secondary。

- [x] T7.1 扩展 TypeScript contracts/IPC 为 provider capability/attribution DTO；默认选择 MusicBrainz，provider switch 清理 results/handles/generation。（final review fixes independently accepted：selected-provider cooldown/status IPC and UI gating。）
- [x] T7.2 重构 `MetadataEditor.tsx`：MusicBrainz 不显示 Connect/Disconnect，标示 Free/no login/TheAudioDB artwork；Spotify 标示 Premium required/optional；Use cover/Fill all 才调用 artwork。
- [x] T7.3 更新三语言、tests 和样例：default no-login、provider switch/stale result、MusicBrainz/TheAudioDB attribution/open、on-demand cover/no automatic lookup、draft-only Save。（M7 re-review fixes：provider-specific no-result/error/rate/timeout parity。）
- [x] T7.4 扩展 security/docs/CI/architecture：精确 MB/TheAudioDB hosts、UA、public key、no arbitrary URL/CSP change；README 说明 MB non-commercial/no key/1rps/商业 MetaBrainz gate 与 Spotify secondary Premium。

- [ ] T8.1 在本 worktree 构建的 macOS app 完成无账号 primary matrix：MusicBrainz search → TheAudioDB on-demand cover → draft-only text/cover/all → MP3+FLAC Save/restart，检查 attribution/no secret/no automatic cover；记录 screenshots/logs。已安装旧 app 或 mock 不可替代。

- [x] T1.1 在 `src/metadata/provider/mod.rs`、新建的 `src/metadata/provider/spotify.rs` 和相关 model tests 中定义 provider-neutral candidate/error 接口与 Spotify config/token/search types；验证 missing-field fixtures。
- [x] T1.2 在 `desktop/src-tauri/Cargo.toml` 引入最新稳定、最小 feature 的 OS credential/random dependencies，更新 `Cargo.lock`、license audit/notice；验证 macOS/Windows locked compile。
- [x] T1.3 新建 `desktop/src-tauri/src/online_metadata/credential_store.rs`，实现 mockable keyring store、versioned secret payload、client-ID fingerprint、read/write/delete 和 no-plaintext-fallback tests。
- [x] T1.4 新建 `desktop/src-tauri/src/online_metadata/auth.rs`，实现 PKCE verifier/challenge/state、`127.0.0.1:0` callback、strict parser、timeout/cancel/denial 和 static callback page；验证 CSRF、wrong path、oversize、duplicate callback。（strict percent + UTF-8 decode、listener stalled/cancel/continue fixtures已补，待 independent re-review。）
- [x] T1.5 新建 `desktop/src-tauri/src/online_metadata/state.rs`，实现 one-active-auth、memory access-token cache、single-flight refresh、rotation、clock skew、6-month/14-day reauth state 和 disconnect；用 mock clock/store/client 验证并发。（injectable service、refresh/401 concurrency fixtures已补，待 independent re-review。）
- [x] T1.6 在 `desktop/src-tauri/build.rs`、online metadata config 和 tests 中接入公开 `UNLOCKM_SPOTIFY_CLIENT_ID`；无配置 build 返回 notConfigured，绝不出现 client secret。

- [x] T2.1 在 `spotify.rs` 实现固定 authorize/token/search endpoints、no-redirect reqwest client、timeouts/body cap/HTTPS host validation 和 cfg(test) local transport；验证 malicious/redirect URLs 不 dispatch。（independent review pass。）
- [x] T2.2 实现 authorization-code exchange 和 refresh-token exchange，正确编码 form、PKCE client ID、access expiry 和 optional rotated refresh token；验证请求/日志不含 client secret 且 error detail 不回显 secret。（independent review pass。）
- [x] T2.3 实现 `type=track` bounded search mapping，limit clamp 到 10，容忍 2026 removed fields，返回 title/artists/album/duration/provider ID 和随机 opaque cover handle；验证 empty/malformed/oversize/malicious image-url fixtures。（independent review pass。）
- [x] T2.4 在 online state/service 中实现 401 refresh-once、429 Retry-After clamp、5xx/offline mapping、search concurrency bound 和 no catalog cache；验证多并发只 refresh 一次。（independent review pass。）
- [x] T2.5 新建 `desktop/src-tauri/src/commands/online_metadata.rs`，实现 provider-neutral status/login/cancel/disconnect/search/open-result/cover-preview commands，只允许固定 provider、validated track ID 与 current opaque cover handle；在 `commands/mod.rs` re-export。
- [x] T2.6 在 `desktop/src-tauri/src/lib.rs` 注册独立 `OnlineMetadataState` 和新 commands，保持 conversion/history state 不变；验证 app setup 在 keyring/client config 缺失时不崩溃。

- [x] T3.1 扩展 `desktop/src/lib/contracts.ts` 与 `desktop/src/lib/ipc.ts`，加入 provider enum、connection status、candidate、rate-limit/error DTO 和 login/search APIs；更新 contract fixture/tests。（final pass：typed stable command errors and immediate authorizing UX covered。）
- [x] T3.2 重构 `desktop/src/features/metadata/MetadataEditor.tsx`，加入 online section 与 query/results state，并确保 dialog open 不调用 search/login network API。
- [x] T3.3 实现候选 Apply：分别填入 title/artist/album、使用 validated cover、或填入全部；保留 local candidate 选择，显示 unsaved feedback；补测试证明这些操作不调用 `updateRunItemMetadata` 或 playback release，Save 才调用。（final pass：candidate generation/provider race covered。）
- [x] T3.4 实现 configured/disconnected/authorizing/connected/reauth/keychain-error、loading/empty/401/429/timeout/offline UI，拆分 search/save alerts 并处理 stale responses。（final review fixes independently accepted：credential-save failure remains credentialUnavailable；status/cooldown provider-scoped。）
- [x] T3.5 实现可访问 result rows、Rust lazy cover preview、Spotify attribution、ID-only Open in Spotify、login cancel/disconnect、focus trap/viewport responsive styles；验证 keyboard、Escape、Tab、矮窗口、object URL cleanup 和 stale preview race。（final review fixes independently accepted：Spotify result row mounts opaque lazy preview and cleans object URL on switch/unmount。）
- [x] T3.6 更新 `desktop/src/i18n/locales/{zh-CN,en-US,ja-JP}.json` 与 parity tests，覆盖登录、重授权、搜索、候选填入、来源、限流、配置和安全存储文案。（final pass：provider-specific locale parity covered。）

- [x] T4.1 扩展 `desktop/src-tauri/tests/security_policy.rs`，断言 CSP/ACL 无新增外网/generic opener 权限、commands 不接受 URL/token、封面 IPC 只接受 handle、source/config/workflows 不含 client secret 或 plaintext credential fallback。（semantic guard added; ready for re-review。）
- [x] T4.2 更新 `README.md`、`README.zh.md`，记录显式联网语义、Spotify app/client ID/redirect URI 配置、Development Mode 限制、凭证存储/断开/6-month reauth、用户逐条封面导入/no persistent cover cache/telemetry。
- [x] T4.3 更新 `.github/workflows/ci.yml` 与 `desktop-release.yml` 的 deterministic tests、公开 client-ID injection/feature-disabled behavior 和 release guard，同时保持 no signing secret、macOS ad-hoc/Windows unsigned contract。
- [x] T4.4 更新 `ARCHITECTURE.md` 的 provider seam、OAuth/token/credential flow、Rust-only network/opener、no-network-on-open、fixed-host 和 secret invariants。

- [x] T5.1 执行 root/desktop Rust、Vite+、Tauri no-bundle、security/license 和 diff 完整自动化，逐项把版本、命令、结果与 warnings 写入本计划 `Progress`/`Completion Evidence`。
- [ ] T5.2 在 macOS native app 使用测试 Spotify account 完成 login/restart/search/text apply/cover apply/all apply/MP3+FLAC save/refresh/disconnect 与 Keychain inspection，记录截图/日志和清理结果。
- [ ] T5.3 在 Windows native app 重复相同矩阵并检查 Credential Manager、DPI/窗口内 OAuth 状态、lazy cover preview 和 installer build；不能用 macOS 证据替代。
- [ ] T5.4 由独立 reviewer 对照本计划检查 provider isolation、token secrecy、callback/SSRF/redirect/rate-limit、policy attribution、原子 Save 和验证缺口；解决 findings 后再推进 `Done`。

## Validation Strategy

### Automated core/provider

- `rtk cargo fmt --all -- --check`
- `rtk cargo clippy --locked --all-targets --all-features -- -D warnings`
- `rtk cargo test --locked --all-targets --all-features`
- `rtk cargo test --locked --test no_network_by_default`
- `rtk cargo test --locked --test metadata_provider_mock`

重点断言：PKCE S256 vectors、state/loopback parser、fixed hosts/no redirects、body/query/limit bounds、2026 missing fields、401 single refresh、429 Retry-After、rotation、secret redaction、打开编辑器不联网、现有 QQ/NCM conversion provider 不回归。另断言 Spotify image URL 永不进入 DTO/IPC；only current random cover handle is accepted; CDN host/no-redirect/body cap/image validation reject spoofed, malformed and oversized images; handles expire and are cleared on new search/disconnect.

### Automated desktop frontend

- `rtk vp -C desktop fmt . --check`
- `rtk vp -C desktop check --no-fmt`
- `rtk vp -C desktop test`
- `rtk vp -C desktop build`

重点断言：contract parity、所有 auth/search states、text/cover/all Apply 都不 Save、Save 仍走既有 IPC、stale search/preview response、candidate limit、attribution/link、object URL cleanup、three-locale parity、focus trap、viewport scroll 和 local cover independence。

### Automated desktop Rust/security/package

- `rtk cargo clippy --locked -p unlockm-desktop --all-targets -- -D warnings`
- `rtk cargo test --locked -p unlockm-desktop --all-targets`
- `rtk cargo test --locked -p unlockm-desktop --test security_policy`
- `rtk cargo test --locked -p unlockm-desktop --test license_audit`
- `rtk vp -C desktop exec tauri build --debug --no-bundle`
- `rtk git diff --check`

测试使用 injected config、mock keyring/opener/clock 和 deterministic loopback servers；CI 不依赖真实 Spotify、用户浏览器或 OS credential UI。configured release build 的 client ID 只通过公开构建配置注入，绝不把真实 token 设为 CI secret/fixture。

### Manual/native QA

- macOS Keychain 与 Windows Credential Manager 分别验证首次登录、重启、access refresh、即将过期提示、重新授权和 Disconnect 删除。
- 验证 consent approve/deny、browser close、callback timeout/cancel、双击 Connect、错误 state、本地端口占用、offline/5xx/429 和凭证库拒绝访问。
- 在已有 title/artist/album、空字段、长 Unicode、多歌手、同名不同专辑/时长和带/不带 album image 的歌曲上搜索、预览并分别应用文本/封面/全部；确认未点击 Save 前文件/history/player 均不变。
- 分别保存 MP3/FLAC，使用外部 tag reader 回读，并确认 source SHA 不变、输出由既有 atomic candidate 发布、失败保留原输出。
- 检查 SQLite、应用日志、IPC payload 和 devtools，确认没有 access/refresh token、authorization code、PKCE verifier/state 或 Spotify image URL；Disconnect 后只保留非敏感应用配置。
- Spotify 结果始终带来源和 track link；专辑封面只能逐条用户触发并作为临时内存 handle/草稿存在，没有 audio preview、catalog cache 或启动/打开弹窗网络请求。

## Risks and Mitigations

- **Spotify 平台条款可能限制把 catalog metadata 或 artwork 写入本地文件。** 首版只做用户主动、逐条、带 attribution/link 的文本与明确选择的专辑封面草稿，不下载 audio、不缓存 catalog；production enablement 前由项目所有者确认 Developer Policy/Terms 和 app use case。若未通过，保持 adapter build-disabled，provider-neutral UI/service 可改接合规来源。
- **Development Mode 无法覆盖公开用户。** 文档明确 Premium owner、5-user 和 2026 quota 限制；公开发布前取得 Extended Quota 或明确限制为 allowlisted preview，不能把本地 QA 推断为普遍可用。
- **refresh token 目前 6 个月后过期。** 保存 issued-at、提前 14 天提示、稳定 reauth state；自动化用 mock clock 覆盖，不把 refresh 当成永久凭证。
- **系统凭证库在 unsigned/ad-hoc preview 中可能弹窗、锁定或行为不同。** 使用跨平台 keyring adapter 与 mockable trait；无安全存储时 fail closed；macOS/Windows native QA 都是完成门禁。
- **loopback OAuth 可能遭遇 CSRF、本地端口抢占或 callback 注入。** 先 bind 后打开浏览器，使用随机 state/PKCE、显式 127.0.0.1、固定 path、bounded parser、single flow 和 timeout/cancel；不使用 localhost/custom scheme。
- **token refresh 并发导致 rotation 丢失或重复请求。** provider state 使用 single-flight refresh，rotation 先写安全存储再切内存；并发 fixture 验证只有一次 refresh。
- **新增联网 UI 可能绕过现有原子 metadata writer。** Apply 永远只改 React draft；仅现有 Save 调用 ID-only `updateRunItemMetadata`。测试以 invocation order 和真实文件回读锁定此边界。
- **远程结果迟到覆盖用户输入。** 前端 generation ID 丢弃 stale response，query/field state 分离；登录断开/搜索错误不重置 metadata draft。
- **Spotify API 继续变化。** response model 对无关字段宽容，对必需的 ID/name/artist/album 缺失逐条丢弃；API limit/refresh lifetime/Development Mode 假设记录在文档和 fixtures，实施/release 时再核对官方 changelog。
- **放宽 CSP、传 image URL 或持久化 artwork 会扩大攻击面。** 所有 HTTP/外部导航/图片 fetch 都在 Rust fixed provider adapter 内；webview 保持无外网 connect/img/generic opener，commands 只接受 provider enum、query、opaque track ID 与 short-lived opaque cover handle。
- **日志/错误泄露 token 或用户 query。** 为 OAuth/provider error 建立 stable codes，日志只记录 provider、阶段和状态；复用/扩展 secret-redaction tests，不记录 query、response body 或 callback parameters。

## Progress

- [x] 2026-08-26：M11 final review **Pass**。Review follow-up 修复 clean no-source 与 network/validation failure 混淆，新增 stable `ArtworkNotFound`/`online_artwork_not_found`，后端保证无 source 不 dispatch image、前端三语明确提示。Full release validation：root fmt/clippy、all targets/features 191，watch 2+2、no-network 1、provider mock 1；desktop clippy、all-targets 90、online 29、security 4、license 1；Vite+ 19 files/125、fmt/check/build；Tauri debug no-bundle/app bundle 全部通过。无 blocker/major/minor/nit。
- [x] 2026-08-26：T12.2 release-ready。0.4.0 no-bundle 与 `.app` 构建成功；Info.plist `CFBundleShortVersionString`/`CFBundleVersion` 均 0.4.0，identifier `world.fansionia.unlockm`，arm64 executable 与 bundled `THIRD_PARTY_NOTICES.md` 存在。仅保留既存 ineffective dynamic import 与 >500 KiB chunk warnings。T8.1 完整 MP3/FLAC Save/restart 与 T5.2-T5.4 Spotify/Windows 外部门禁继续保留，不阻止本次用户明确要求的 v0.4.0 tag/push。
- [x] 2026-08-26：M11 native primary path passed。当前 debug `.app` 搜索 `Jackie toconoma` 返回第一条 TENT（RGID `29e467dd-...`），Use cover 通过 CAA/Internet Archive 受限跳转取得真实 TENT 图片，左侧 preview/source/unsaved feedback 正确；Cancel 后重开显示“尚未选择新封面”，未写文件/history。Review follow-up 增加 `ArtworkNotFound`/`online_artwork_not_found`，确定性测试区分 clean no-source 与网络/验证失败。
- [x] 2026-08-26：T12.1 complete。`Cargo.toml`、desktop Rust/package/Tauri config 与 workspace `Cargo.lock` 的 `unlockm`/`unlockm-desktop` 均为 0.4.0；历史依赖版本未机械改写。无新 crate/npm dependency、credential 或 release secret。
- [x] 2026-08-26：T11.1-T11.3 implemented。新增 `CoverArtArchiveClient`（HEAD no-follow lookup、validated archive location、最多 3 个受限 HTTPS redirects、10s/20s、cap/validate）与 `FreeArtworkClient` composite。TADB 接受真实 `album:null`、raw trimmed query/normalized exact comparison、10s/20s。Desktop state 使用 CAA primary/TADB fallback 并按 trusted host fetch；capability/三语/docs/architecture/security 更新。Deterministic validation：CAA 6、TADB 13、composite 5、provider 44、desktop online 28、security 4、frontend metadata+locale 38、check/fmt passed。
- [x] 2026-08-26：用户要求修复全部 cover findings 并发布 v0.4.0。诊断证据：当前 TADB 3s connect 在本机 5.23s TLS 前稳定 timeout；10s/20s 对 Parachutes 5.54s 返回 200。真实未收录为 `{"album":null}`，现 `Vec<Album>` 将其误判 malformed 并阻断 fallback；raw `Daft Punk/Random Access Memories` 命中而当前 normalized `daftpunk/randomaccessmemories` 返回 null。TENT 在 TADB MBID/raw search 均无记录，但 CAA release-group endpoint 返回 release `906ce501-...` 的 front 250/500/1200。打开 M11/M12，计划进入 In Progress；remotes `origin`/`github` 已确认，`v0.4.0` 不存在。
- [x] 2026-08-26：M10 implementation complete and ready for review。用户追加的 local cover grid finding 已修复：真实 debug `.app` 左 pane 显示稳定三列，两行中/英/日 album name 均未被图片或下一行覆盖。宽屏浅色/深色 split workspace、fixed actions、empty/results、独立 pane scroll 通过原生视觉 QA；theme 已恢复、草稿 Cancel、app 退出。TheAudioDB timeout 在顶部/候选可见，且修正为独立封面服务失败文案。Targeted editor 34/34、full Vite+ 17 files/102 tests、fmt/check/build、provider 32、desktop online 28、security 4、diff check 均通过；一次 Node type reference check failure 已局部修复并复验通过。状态恢复 In Review。
- [x] 2026-08-26：M10 native visual QA finding。宽屏 split/results UI 通过，真实 Jackie query 显示 10 个字段化候选；TheAudioDB 首次 cover fetch timeout 已在列表上方和候选内可见，但错误归因仍误写为 MusicBrainz，已改为独立 online-cover failure 文案。用户截图进一步发现 local candidate image intrinsic height 溢出 grid row、遮挡后续卡片和 album name，打开 T10.5；M10 继续 In Progress。
- [x] 2026-08-26：T10.1-T10.3 implemented。`MetadataEditor` 改为 split-workspace DOM，左侧 metadata/cover draft、右侧 provider/search/labeled result list；CSS 使用现有 semantic tokens、1120px dialog、独立 pane scroll、860px/560px 单栏回退和 reduced-motion skeleton。候选使用 `dl` 显示 6 个字段与 mm:ss 时长，头部反馈不再落在长列表末端。online cover 成功生成左侧 preview object URL/来源/未保存状态，失败显示顶部与候选 alert，upload/local replacement 清理 preview。三语文案已补；targeted editor 33/33 passed，Vite+ check passed。
- [x] 2026-08-26：用户原生验收新增 M10，计划由 In Review 回到 In Progress。Design audit（preserve mode，variance 4 / motion 2 / density 6）确认：460px 单列 dialog 让 MB 无预览结果仍占 3.5rem 空列，字段点分隔文本在窄列逐词折行，反馈位于十条结果之后；`applyCover` 只写 `cover` bytes/`existingArtworkId`，未设置 `preview` 或成功来源，导致操作不可见。目标为现有 token/字体/主题内的桌面双栏，不新增依赖或网络能力。
- [x] 2026-08-26：M9 final re-review **Pass**。Major relevance finding 已修复且原生复验通过；没有剩余 blocker/major/minor/nit。修复后独立重跑 root fmt/clippy/all-targets-all-features（180 tests）、no-network/mock gates、desktop clippy/all-targets（87 tests）/security（4）/license（1）、Vite+ fmt/check/full tests（17 files/99）/production build、Tauri debug no-bundle build 和 diff check，全部通过。Build 仅有既存 ineffective dynamic import 与 >500 KiB chunk warnings。计划保持 In Review 仅因 T8.1 与 T5.2–T5.4 原有外部/跨平台门禁，不因 M9 finding。
- [x] 2026-08-25：T9.4 Major relevance finding resolved and ready for final re-review。官方 API 文档确认 `dismax=true` 即 plain indexed search 并自动转义特殊语法；客户端删除手工 Lucene escape 与 `recording:(...)` title-only 包装，保留 trim/200 scalars/URL encoding/limit10/1rps。request fixture 断言 `query=a%2Bb` 且不存在 `recording%3A`；MB 9、provider 32、desktop online 28、security 4 passed。重建 debug `.app` 后，歌手栏 focus 3s 稳定，`relive toconoma` 约 8s 返回，第一条为 `Relive / toconoma / TENT / 301s`。QA 使用 Cancel 且退出 app，未写媒体文件。
- [x] 2026-08-25：M9 review Needs Changes（Major relevance）。完整自动 review 门禁均通过，原生 debug UI 也证明焦点在歌手栏稳定保持；但首次 MB 请求收到可恢复 503 后，重试虽返回 10 candidates，却全部是通用 “Recording” 结果。对照 API 发现当前 `recording:("relive toconoma")` 把 title+artist 输入错误限制在 recording/title 字段，且与 dismax 组合产生错误匹配。打开 T9.4，计划回到 In Progress；当前 QA 草稿已 Cancel，未保存或改写媒体文件。
- [x] 2026-08-25：M9 user-acceptance regression fixes complete and ready for review。全量前端 17 files/99 tests、fmt/check passed；root provider 32、desktop online metadata 28、desktop cargo check、Rust fmt 与 diff check passed。计划恢复 In Review；未改变 T8.1 与 T5.2–T5.4 的既有原生/外部 QA 缺口。
- [x] 2026-08-25：T9.1/T9.2 implemented。初始 title focus 改为 mount-only effect；Escape/Tab listener 只安装一次并通过 ref 调用最新 `onClose`。新增 rerender regression 证明 artist input 保持焦点且 Escape 不调用旧 callback；`MetadataEditor.test.tsx` 31/31 passed。MusicBrainz 新增 10s `CONNECT_TIMEOUT`（20s total 不变）与常量断言，provider 9/9 passed。修改后的 exact `MusicBrainzClient` 首次在 5.80s 收到服务端 unavailable（不再 timeout），随后以 `relive toconoma` 在 6.89s 成功返回 10 candidates；诊断 example 已移除。
- [x] 2026-08-25：用户原生验收发现两个回归，计划由 In Review 重新进入 In Progress。焦点根因是初始 `titleRef.focus()` 与依赖 `onClose` 的键盘 listener 共用 effect；`HistoryDetail` 每次 render 都创建新 callback，导致 effect 反复把焦点抢回标题。MusicBrainz 根因由 exact client probe 复现：当前 `MusicBrainzClient` 在 3.007s 返回 `RequestTimeout`；同机 TUN 直连和显式 `127.0.0.1:7897` 代理在放宽 connect bound 后均约 5.52s 完成 TLS、5.73s 收到 HTTP 响应，证明遗留的 3s `connect_timeout` 在 20s total timeout 之前提前截断请求。Tauri command 已使用 `spawn_blocking`，无需改变执行模型。

- [x] 2026-08-25：M6 T6.1/T6.2 完成。`MusicBrainzClient` 固定 `https://musicbrainz.org/ws/2/`，发送 `UnlockM/<version> (https://git.fansionia.world/fansion/unlockm)` User-Agent，使用 process-wide `OnceLock` 1rps gate（scheduler trait 可注入）；recording 搜索固定 no-redirect、3s connect/8s total timeout、512KiB capped streaming、limit 1–10、Lucene escape。local TCP fixture 验证 UA/query/limit/scheduler 与 503，mapping tests 覆盖 joinphrase、recording+release-group dedupe、missing release fallback/duration。`rtk proxy cargo test --locked -p unlockm metadata::provider::musicbrainz -- --test-threads=1` passed (4 tests)。
- [x] 2026-08-25：M6 READY FOR REVIEW。完成 T6.3–T6.5：TheAudioDB 固定 public key `123`、API/image exact hosts、no redirect、bounded read 和 `validate_cover`；local fixtures 覆盖畸形 image bytes、无 Content-Length 的 chunked MAX+1、image/API 302 及 production-host spoof pre-dispatch（server broken pipe 忽略）。`OnlineMetadataState` 新增可注入 free-provider seam：MB search 不会自动 TheAudioDB lookup/fetch；同 handle lookup 合并，lookup 完成后图片复用两许可 preview budget；new search、Disconnect、TTL 和 post-fetch generation/binding 均拒绝旧 bytes。capability contract 证明 MusicBrainz primary_order=0/no-auth/TheAudioDB attribution，固定 recording opener；Spotify 仍 secondary。
- [x] 2026-08-25：M7 READY FOR REVIEW。editor 从 Rust capability IPC 读取 no-auth primary order，默认 MusicBrainz；provider switch 清理 generation/results，MB 显示 Free/no-login、不会显示 Spotify auth controls，TheAudioDB artwork 只从 Use cover/Fill all 发起。Spotify 明示 Premium-required optional。三语言、draft-only/stale/on-demand tests、精确 host/UA/public-key security assertions、README/README.zh/ARCHITECTURE 已更新。
- [x] 2026-08-25：M6/M7 independent review Needs Changes；T6.1/T6.2/T7.1/T7.3 reopened。要求补齐 MB `dismax=true`、可注入 production 1rps time semantics、spoof/timeout dispatch tests、clean artist DTO + Rust-only credited fallback hint、provider-bound cover handles/preview command、TheAudioDB fallback bounds；并要求 provider switch 使全部 deferred async state 安全失效、Spotify cooldown 不阻塞 MB，及三语 provider-specific result/error 文案。
- [x] 2026-08-25：M6/M7 FIXES READY FOR RE-REVIEW。MusicBrainz request 增加 `dismax=true`；真实 `OneRequestPerSecond` 以 injected clock/sleeper 验证 reserve/start（无 fixed sleep），新增 production root spoof pre-dispatch 与 slow timeout fixture。DTO `artists` 仅 clean names，`credited_artist`/release-group 均 Rust-only，TheAudioDB fallback 使用完整 credited string，且 artist/album 限制 200 scalars/4096 encoded URL。cover binding 绑定 issuing provider，IPC preview 再传 provider 并拒绝 mismatch；Spotify 429 cooldown 仅 Spotify。前端以 provider ref+generation 丢弃 switch 后的 search/cover success/error/finally，三语言均有 MusicBrainz/Spotify 具体 no-result/rate/unavailable/timeout/malformed 文案。定向验证：MusicBrainz 9、TheAudioDB 11、desktop online state 26、MetadataEditor 28、security_policy 4 均通过，且 fmt/diff check 通过。
- [x] 2026-08-25：M3 final implementation evidence refreshed：provider-neutral contract/IPC、即时 authorizing、draft-only apply、provider+generation stale search/cover suppression、provider-scoped cooldown、keyboard/focus/viewport/object URL 与三语 typed status coverage 均在 `MetadataEditor.test.tsx`（28 tests）通过；T3.1/T3.3–T3.6 closed pending final independent plan review.
- [x] 2026-08-25：M3 final Pass。最终 state/contract/UX acceptance is covered by the current `MetadataEditor.test.tsx` 28 tests and full Vite+ suite 94 tests; no remaining M3 finding is open.
- [x] 2026-08-25：M4 T4.1 semantic security guard added and ready for re-review：不只查 literal，而是断言 command 参数只有 fixed provider/query/opaque handle、opener 必须经 Spotify track/MB recording validators、Rust-only release-group hint 不序列化、credential store 无 fs/localStorage fallback、fixed provider hosts/no redirect、CSP/ACL 禁止外网与 generic opener、release/build 只允许 public client-ID variable 并拒绝 client-secret path。`security_policy` 4 tests passed.
- [x] 2026-08-25：M6 independent re-review Pass with Notes：`metadata::provider::musicbrainz` 9、TheAudioDB 11、desktop `online_metadata` 26、`security_policy` 4、fmt/diff all passed. 唯一 minor 是 200 scalars 的高字节 Unicode 仍可能让 percent-encoded fallback URL 超过 bound；已立即添加 exact fixture：两侧各 200 个 U+20000 在 scalar 校验通过后触发 4096-byte URL rejection，nonblocking listener 证明 zero dispatch。`metadata::provider::theaudiodb` now passes (12). **M6 MINOR FIXED**.
- [x] 2026-08-25：M6 final Pass。minor fixture re-review accepted: `rtk cargo test --locked metadata::provider::theaudiodb -- --test-threads=1` passed (12) and diff check passed; no M6 finding remains open.
- [x] 2026-08-25：M7 independent re-review Pass：provider switch/state/error/locale boundary accepted. Review evidence: `MetadataEditor.test.tsx` 28, full Vite+ suite 94, desktop online state 26, root provider 31, frontend check/fmt and diff check passed. **M7 READY**.
- [x] 2026-08-25：M4 SEMANTIC FIX READY FOR RE-REVIEW。security gate now parses and exact-matches CSP connect/img/media directives; checks release public-variable interpolation and zero secrets interpolation; serializes a real candidate to prove Rust-only RGID/credited artist do not cross IPC; asserts keyring/no-fallback; and imports exact MusicBrainz root/host/meaningful UA and TheAudioDB API/image/public-123 constants in addition to no-redirect code paths. `rtk cargo test --locked -p unlockm-desktop --test security_policy` (4) and diff check passed.
- [x] 2026-08-25：M4 LAST FIX READY FOR FINAL REVIEW。Spotify now exports the exact accounts/API/open hosts, token/search/authorize endpoints and two image hosts used by the adapter; security test directly imports/equality-checks those values and a validated fixed open-track URL. Credential-store source is explicitly banned from rusqlite/file/path/env/serde-reader-writer fallbacks. `security_policy` (4), `license_audit` (1), fmt and diff check passed.
- [x] 2026-08-25：M4 independent final review Pass。CSP/IPC/DTO/release-env/keyring-only and all fixed Spotify/MusicBrainz/TheAudioDB constants are accepted as implementation-bound semantic guards. Independent checks: `security_policy` (4), `license_audit` (1), blank/configured public-ID release guard and diff check passed. M4 is closed.
- [x] 2026-08-25：M8 READY FOR REVIEW（automated complete; live no-account matrix has an explicit environment gap）。全量 root fmt/clippy/all-targets/all-features、serial watch_recursive/watch_stability、no_network_by_default、metadata_provider_mock 均通过；desktop clippy/all-targets/security/license、Vite+ fmt/check/full test (17 files/94 tests)/build、macOS feature-disabled `vp -C desktop exec tauri build --debug --no-bundle` 均通过。Vite build only emits pre-existing ineffective dynamic import and >500 KiB chunk warnings. 尝试用 macOS UI QA 时可控的 `world.fansionia.unlockm` 是已安装旧版本，编辑器不包含本分支 online section；为避免干扰用户运行中的 app，未重启/替换它，因此没有发起真实 MusicBrainz/TheAudioDB 请求或 claim live pass。
- [x] 2026-08-25：M5 AUTOMATED READY FOR REVIEW。T5.1 completed; macOS no-bundle artifact is built at `target/debug/unlockm-desktop` with missing Spotify client ID feature-disabled path. T5.2（真实 macOS free-primary/Spotify account/keychain）、T5.3（Windows）和 T5.4（final independent review）保持未勾选：没有可验证的本分支 native live session、没有 Spotify Premium/allowlisted account，也没有 Windows environment；不得用 mocks、旧 installed app 或 macOS build 替代。
- [x] 2026-08-25：final review Needs Changes；T3.4/T3.5/T7.1 and new T8.1 opened. Required changes are: Spotify result must visibly mount opaque-handle `OnlineCover`; OAuth exchange followed by secure-store save failure must surface `credentialUnavailable`, not reauthorization; cooldown is truly provider-scoped in state/status IPC/UI (MB 429 does not block Spotify and inverse); native no-account macOS matrix becomes an explicit unchecked task.
- [x] 2026-08-25：FINAL FIXES READY FOR RE-REVIEW。Spotify rows now mount `OnlineCover` only with an opaque handle; integration coverage proves preview IPC receives only `spotify` + handle, creates a blob URL, and revokes it on provider switch. `complete_login` now sets reauthorization only for authorization/invalid-grant failures; successful exchange + failing keyring save deterministically remains `credentialUnavailable`. State stores separate Spotify/MB retry deadlines; `status_for(provider)`/IPC select the provider; 429 tests prove each provider blocks only itself; UI fetches selected status and displays/disables MB cooldown without disabling Spotify. Validation: desktop online state (28), desktop all targets (87), root provider (32), `security_policy` (4), targeted editor (30), full Vite+ suite (95), desktop clippy/check/fmt and diff check all passed. T3.4/T3.5/T7.1 remain unchecked until independent re-review; T8.1/native gap remains unchecked.
- [x] 2026-08-25：final independent re-review Pass with Notes。T3.4/T3.5/T7.1 are accepted and closed: Spotify visible opaque lazy preview/revocation, credential-store failure state, and provider-scoped cooldown/status all meet scope. Independent evidence: full Vite+ suite (96), desktop online state (28), root provider (32), `security_policy` (4), desktop clippy and diff check passed. Overall status remains **In Review** exclusively for T8.1/T5 native/external QA and final release disposition; those tasks remain unchecked.
- [x] 2026-08-25：native QA partial evidence（T8.1 remains unchecked）。通过 `RUSTC=/opt/homebrew/opt/rustup/bin/rustc` 保持 sccache 成功构建合并 main `027f9e3` 后的 debug app；此前 `sccache rustc -vV` EPERM 已记录。QA 临时目录 `/private/tmp/unlockm-native-qa.QLnpKL` 保留：converted/copy MP3+FLAC、默认 MusicBrainz/no-login/provider-timeout UI、draft 修改后 Cancel→reopen unchanged、MP3 save（Yellow QA/Coldplay QA/Parachutes QA，733B→1.8KB）和 FLAC save values、restart history/tag persistence、tag string inspection 均通过；SQLite `rg -c token|verifier|client_secret` 无匹配。QA history record 与临时文件均保留，未执行删除。
- [x] 2026-08-25：MusicBrainz production timeout revised from 8s to 20s（constant plus unit assertion；targeted `metadata::provider::musicbrainz` 9 passed）。原因：elevated `curl` to the same public endpoint returned HTTP 200 in 11.150974s，8s 不足以代表可用网络。GUI child 的受限网络下 broad/narrow live MB searches both timed out；20s rebuild 后仍 timeout，因此 live results、TheAudioDB on-demand cover、attribution from a real result remain unverified. This is an environment network gap, not a pass/fail inference about the public provider.
- [x] 2026-08-25：M6 capability contract implementation added `OnlineMetadataProviderCapability` (primary order MusicBrainz=0 / Spotify=1, `requiresAuthorization`, `artworkSource`, attribution) and `online_metadata_provider_capabilities` IPC; candidates carry the same safe capability/attribution fields. `rtk proxy cargo test --locked -p unlockm-desktop online_metadata -- --test-threads=1` passed (19 state/auth tests plus security subset); `rtk proxy cargo check --locked -p unlockm-desktop`, `rtk vp -C desktop check --no-fmt`, `rtk proxy cargo fmt --all`, `rtk vp -C desktop fmt .`, and `rtk git diff --check` passed.

- [x] 2026-08-25：读取 `AGENTS.md`、父级 `RTK.md`、`docs/PLANS.md`、完整 `ARCHITECTURE.md`、active desktop metadata UX plan、CI/release/CSP/security 配置及相关 React/Rust/SQLite/provider 代码。
- [x] 2026-08-25：确认没有覆盖“编辑器内在线搜索 + Spotify OAuth/credential persistence”的 active plan；既有 In Review 计划只提供本地编辑器基础且明确把外部 provider 置于当时 Non-Goals。
- [x] 2026-08-25：核对 Spotify 官方 PKCE、redirect URI、refresh-token lifetime、Search、February 2026 migration、rate-limit 和 Developer Policy；将动态 loopback、6-month reauth、limit 10、Development Mode/政策门禁纳入设计。
- [x] 2026-08-25：创建本 Ready ExecPlan；本阶段未修改 production source、tests、build scripts 或 runtime configuration。
- [x] 2026-08-25：Implementation 开始；按 M1 → M5 顺序执行，每个 milestone 完成后由独立 reviewer 并行审查。真实 Spotify account、Keychain/Credential Manager 和 Windows 原生 QA 仅能作为外部验证缺口记录，不能由 mock 替代。
- [x] 2026-08-25：M1 完成。新增 provider-neutral Spotify DTO/PKCE authorization flow、OS credential-store abstraction 和 independent `OnlineMetadataState`；`cargo check -p unlockm-desktop --locked` 通过，`cargo test -p unlockm-desktop online_metadata --locked -- --test-threads=1` 通过。原生 Keychain/Credential Manager 与真实 OAuth QA 仍待 M5。
- [x] 2026-08-25：根据用户新增范围，暂停 M2 编码并将专辑封面纳入计划：Rust-only random handle + bounded short-lived memory map + lazy validated fetch；不向 webview 传 image URL，不放宽 CSP，不建立 cover/catalog persistence。
- [x] 2026-08-25：收到 M1 review 的 Needs Changes 后暂停 M2，实施待复验修正：refresh response 未 rotation 时保留原 `issued_at_ms`，通过 injected deterministic clock 覆盖生命周期；callback connection 有 150ms read timeout、2s bounded incremental request-line deadline/cancel checks，错误 callback 不消耗合法 flow；拒绝 duplicate `state`/`code` 与 malformed percent encoding；credential/token Debug redaction；generic provider seam moved to `provider/mod.rs`；keyring notice/license regression added。
- [x] 2026-08-25：M2 完成，加入 fixed Spotify authorize/token/search client、no redirect/time/body/query bounds、refresh-once/429 state handling、ID-only external opener 和 Tauri commands。新增封面 security boundary：search adapter 仅在 Rust 内保留 exact Spotify CDN image URL，state 为当前最多 10 个候选签发 128-bit opaque handles，new search/Disconnect 清理，preview command 只接受 handle 并重新验证/下载/`validate_cover` bytes；候选 DTO 不含 image URL。
- [x] 2026-08-25：M1 second review found remaining strict-decoding and listener/state-fixture gaps. T1.4/T1.5 are deliberately reopened and M2 follow-on work is paused until independent re-review accepts their fixes.
- [x] 2026-08-25：M1 second fixes implemented, awaiting independent re-review: custom strict query decoder rejects malformed `%` and invalid UTF-8 instead of lossy replacement; listener fixtures cover bad-then-good and stalled-read cancellation; `OnlineMetadataState` owns an injectable `SpotifyService`, so mock tests cover concurrent single refresh and 401→refresh-once→reauthorization lifecycle.
- [x] 2026-08-25：M1 final acceptance-gap fix implemented, awaiting independent re-review: callback/global timeout durations are injectible; deterministic fixtures prove a partial stalled connection reaches its per-connection deadline without consuming the listener, a following valid callback succeeds, and the short global deadline exits without waiting for the production 3-minute limit.
- [x] 2026-08-25：M1 independent final review passed. T1.4/T1.5 strict parser/listener and injectable state fixtures are accepted; review commands include the deterministic auth subset and full `online_metadata` subset recorded below.
- [x] 2026-08-25：M2 independent review found streaming cap, error/cooldown propagation, handle generation/race, local transport fixture and documentation drift issues. T2.1–T2.4 are reopened; T3.1 contracts/IPC and editor work exists but is not M3-complete and will remain unclaimed until M2 is re-reviewed.
- [x] 2026-08-25：M2 fixes in progress: response reads now use capped streaming `Read::take(MAX+1)`; cfg(test) local endpoints verify PKCE form contains no client secret and chunked/no-content-length oversize bodies are rejected; state records refresh/initial/replay rate-limit cooldown and generation-checks cover handles. Remaining re-review fixtures include complete invalid_grant/429/race matrices.
- [x] 2026-08-25：M2 re-review submission prepared: added local redirect/malformed/oversize cover fixtures; mocked invalid_grant, refresh/initial/replay 429 and one-refresh concurrency; generation/TTL handle invalidation fixture; all code results remain pending independent re-review.
- [x] 2026-08-25：M2 second re-review still Needs Changes. T2.1–T2.4 deliberately reopened for disconnect-vs-inflight token publication, post-fetch handle revalidation/preview concurrency, and complete production-adapter error/host fixtures. M3 remains unclaimed.
- [x] 2026-08-25：M2 second-fix implementation complete and awaiting third independent review. `disconnect` serializes with request/refresh publication and blocked-refresh fixture proves no credential/access resurrection; preview has a two-permit gate plus post-fetch binding/generation/TTL recheck and blocked fixture proves third preview waits. Added production host-spoof no-dispatch and local token/API 401, Retry-After clamp, 5xx/offline fixtures. T2.1–T2.4 remain unchecked until reviewer accepts them.
- [x] 2026-08-25：M2 final race fix implemented and awaiting review: AuthorizationFlow carries an auth generation; `complete_login` shares request gate, validates current flow both before and after injected code exchange, and Disconnect invalidates the generation. A blocking exchange fixture proves that code received/token response blocked → Disconnect → response release leaves no credential/access and `Disconnected` status. API host-spoof no-dispatch fixture added alongside accounts host check.
- [x] 2026-08-25：M2 final independent review passed with notes. T2.1–T2.4 accepted; production fixed-host, streaming bounds, session/generation isolation and opaque cover invariants are verified. Overall plan remains In Progress for M3–M5.
- [x] 2026-08-25：M3 implementation complete and awaiting independent review. Provider-neutral contracts/IPC, explicit no-network-on-open editor state, connection controls, bounded query/results, Rust-only lazy cover previews, Apply draft boundary, attribution/open action, three-locale text, credential-unavailable state and focus/viewport preservation are covered by frontend tests.
- [x] 2026-08-25：M4 implementation complete and awaiting independent review. Security policy now locks secret/URL/image-URL/CSP boundaries; bilingual docs cover explicit Spotify operation/configuration/storage/policy; CI runs the security gate and release injects only a public client ID; architecture documents Rust-only online provider and cover-handle invariants.
- [x] 2026-08-25：M3 review Needs Changes. T3.1/T3.3–T3.6 are reopened for immediate authorizing UX, typed stable errors/cooldown, stale candidate/cover cleanup, explicit responsive styling and expanded accessibility/race tests.
- [x] 2026-08-25：M4 early review found T4.1 claim unsupported and Development Mode documentation insufficient. T4.1 reopened pending static online-boundary/workflow assertions; README clarification will name Premium owner, max 5 users and Extended Quota release gate.

## Discoveries

- 用户没有 Spotify Premium；Spotify Development Mode 的 Premium owner/最多 5 用户约束使其不能作为当前用户可用的默认搜索路径。MusicBrainz + TheAudioDB 将作为无需登录的 primary 方向，具体 endpoint、rate limit、attribution、cover policy 和 license 仍待官方约束核对。

- 现有编辑器已经把草稿编辑与最终文件 mutation 分开；在线候选只需进入草稿即可天然复用原子写入、播放器 release/restore 和 history reconciliation，无需新建第二条 tag pipeline。
- 现有 webview CSP 完全禁止外部 HTTP/image，且 security test 明确锁定该边界。由 Rust 搜索、结果只传文本、Rust 按 track ID 打开固定 Spotify URL，可在不放宽 CSP 的情况下完成首版。
- Spotify 对 native/public client 明确推荐 PKCE；client secret 不适合也不应进入桌面 app。可持久化的真正 credential 是 refresh token，而 client ID 是公开构建配置。
- Spotify 2026 文档把 Developer Dashboard refresh token 生命周期改为 6 个月，不能沿用“refresh token 永久有效”的旧实现假设。
- Spotify loopback 允许动态端口，但要求注册显式 IP literal 且禁止 `localhost`；现有 Vite dev server 的 127.0.0.1 端口不能复用为 OAuth callback ownership。
- Spotify Development Mode search limit 已降至 10，且部分 popularity/market/profile fields 移除；初版正好可用 bounded 10-result UI，并应避免任何 popularity ranking。
- Development Mode app 的 Premium owner/5-user 限制使“内置共享 client ID 即可公开发布”不成立；Extended Quota 或明确 allowlist 是外部 release gate，不是代码测试可替代的事实。
- Spotify Policy 对 metadata/cover 展示、链接和独立服务有限制。首版只允许用户逐条把 Rust-validated cover 放入既有编辑草稿、不缓存 catalog，并保留 provider adapter 可替换性，可减少但不能代替项目所有者的条款确认。
- 现有 `AppState` 的 active mutex 专为 conversion/history file mutation；OAuth/search 不应占用它。单独的 `OnlineMetadataState` 可以独立限制 auth/search，同时最终 Save 仍受 history mutation guard 保护。
- `keyring` 4.1.6 是当前稳定版并在 macOS 使用 Apple native store、Windows 使用 native Credential Manager store；直接启用其 v1 default API 保留同步 `Entry` 访问，避免应用自己引入明文持久化。
- 用户明确要求 Spotify search 支持专辑封面；原始 image URL 具有 SSRF/泄露风险，因此它不能成为 DTO/IPC 字段。随机 handle 与 Rust-only short-lived binding 可在维持 lazy UX 的同时保留 fixed-host/validated-byte 边界。
- Refresh token 的 issued-at 是授权生命周期的安全边界：access refresh 没有返回新 refresh token 时不能重置它；只有 Spotify rotation 返回新 refresh token 才重置 6-month 计时。
- 当前可由 Computer Use 发现的 `world.fansionia.unlockm` 是已有安装实例而非本 worktree 刚构建的 debug artifact；其 metadata editor 没有在线搜索区域。为了不关闭或覆盖用户的 live application，原生 free-provider QA 只能记录为未执行，不能从该实例推断本分支结果。
- 合并 main 后的 debug app 可构建并运行，但其 GUI child network 在本 QA environment 中仍无法完成 MusicBrainz 请求；同端点 elevated curl 约 11.15s 返回 200。20s client timeout 使正常慢响应不被过早截断，却不能绕过 GUI child network restriction。TheAudioDB cover test 依赖 live MB result，因此同样被阻断。
- 用户当前启用了 TUN，且同机直连与显式系统代理对照具有相同的约 5.52s TLS 建连时间；实际失败不是缺少代理发现，而是 `connect_timeout(3s)` 先于 20s total timeout 触发。`reqwest` connect timeout 覆盖 DNS/TCP/TLS 建连，因而此前只提高 total timeout 的修复不完整。
- 编辑器的 focus trap 边界本身只在 Tab 到首尾元素时循环；持续抢焦点来自初始 focus 与依赖不稳定 `onClose` callback 的 listener effect 错误耦合，而不是 trap 的 Tab 分支。
- MusicBrainz 编辑器 query 是 title、artist、album 的组合自由文本，不应整体限定为 `recording` title 字段。原生 live 结果证明 `recording:("relive toconoma")` + dismax 会返回通用 “Recording” 候选；同一 endpoint 的 free-text dismax 查询能跨 recording/artist 等默认字段获得相关结果。
- 当前 MusicBrainz result 没有自动图片却仍使用预留 3.5rem 图片列；在 460px modal 中有效文本宽度过窄，点分隔 attribution 会逐词换行。封面成功 bytes 只存在 React draft，现有 `preview` 仅由 file input 更新，因此用户无法确认远程封面是否成功应用。
- local artwork 图片的 intrinsic aspect-ratio 高度大于未约束的 outer grid auto row；`max-block-size` 只裁剪容器、不限制 card/image，导致图片跨行覆盖且单行 ellipsis title 被下一行遮挡。需要显式 grid row/card/image block size 与 overflow containment。
- TheAudioDB production response uses `album:null` for no match, not `album:[]`; serde `default` on `Vec` does not accept null. TheAudioDB text search is sensitive to spaces, so deleting them before dispatch turns known multiword matches into false negatives. On the current TUN route, its TLS setup also exceeds the inherited 3s connect timeout.
- Cover Art Archive already has TENT artwork for MusicBrainz release-group `29e467dd-f441-44ad-a7a2-8863c37c464e`, redirecting through exact `archive.org` storage to a dynamic `*.archive.org` host. CAA is structurally better aligned with retained MusicBrainz IDs than TheAudioDB and can remain fully Rust-only/on-demand.

## Decision Log

- **2026-08-25 — D1:** 新建独立增量计划，不修改 `2026-08-23-desktop-history-player-metadata-ux.md`；其编辑器/原子写入实现是本计划依赖。
- **2026-08-25 — D2:** 首版 provider 为 Spotify，但 IPC/UI/credential state 使用 provider-neutral 边界，避免把 OAuth 或搜索 response 直接写进 `MetadataEditor`。
- **2026-08-25 — D3:** 桌面登录使用 Authorization Code with PKCE + `127.0.0.1` dynamic loopback；不使用 client secret、implicit flow、localhost、custom scheme 或前端 callback。
- **2026-08-25 — D4:** refresh token 只存 OS credential manager，access token/verifier/code/state 只存 Rust memory；无 plaintext fallback，无 SQLite migration。
- **2026-08-25 — D5:** 搜索只能由用户主动触发，单次最多 10 条；打开应用/编辑器不联网，不建立 catalog cache。
- **2026-08-25 — D6:** 选择候选只填 title/artist/album 草稿，现有 Save 是唯一文件 mutation；不自动匹配或自动保存。
- **2026-08-25 — D7 (superseded by D11):** 首版不获取或嵌入 Spotify cover/audio preview，保留本地封面能力，CSP 不加入外部 img/connect source。
- **2026-08-25 — D8:** OAuth/result 外部导航由 Rust 固定 provider adapter 构造；webview 不获得 generic opener 或 URL input。
- **2026-08-25 — D9:** client ID 作为公开 compile-time config；缺失时 feature-disabled build 仍可用。公共发布需单独满足 Spotify quota/account/policy gate。
- **2026-08-25 — D10:** 计划可开始实现 provider/PKCE/UI 基础，因此状态为 Ready；Spotify production enablement 保留为显式 release acceptance criterion，而非隐藏阻塞。
- **2026-08-25 — D11:** 用户要求加入 Spotify 专辑封面。封面只支持用户逐条主动导入：search adapter 将已验证的 Spotify CDN URL 绑定至随机 opaque handle，webview 仅以 handle 获取 Rust 懒加载、`validate_cover` 验证后的 bytes；new search/TTL/Disconnect 清理内存 binding，Save 前绝不写文件。
- **2026-08-25 — D12:** 用户确认 Spotify 搜索需要 Premium 且当前不可用。Spotify 降为明确 Premium requirement 的 optional secondary provider；计划的默认/primary 路径改为无登录 MusicBrainz 文本 metadata 与 TheAudioDB 专辑封面。等待官方 API 约束核对后再细化实现任务。
- **2026-08-25 — D13:** 使用 `keyring` 4.1.6 的 native v1 API 作为 Spotify optional login 的唯一 persistent credential backend；其 API failure 映射为 `credential_unavailable`，不实现 file/SQLite fallback。
- **2026-08-25 — D14:** MusicBrainz production total timeout 设置为 20s，而非测试/local transport timeout；本轮外部 endpoint 的已授权 curl evidence 为 11.150974s HTTP 200。该调整只避免把可用但慢的 public response 误分类为 timeout；response cap、fixed host、no redirect 和 1rps gate 不变。
- **2026-08-25 — D15:** 初始标题聚焦改为 mount-only effect；Escape/Tab listener 通过 callback ref 读取最新 `onClose`，使 listener 无需因父组件 callback identity 改变而重新安装或重跑初始 focus。
- **2026-08-25 — D16:** MusicBrainz production connect timeout 从 3s 提高到 10s，并继续由 20s total timeout 封顶。该值覆盖本机已观测约 5.52s TLS 建连而仍保持断网有界；不新增 proxy adapter、不放宽目标 host/redirect/body/rate 安全边界。
- **2026-08-25 — D17:** MusicBrainz 搜索改用 trim 后的 plain-text query 并保留 `dismax=true`，不再手工 Lucene escape 或把组合 title+artist+album 输入包装为 `recording:(...)`。MusicBrainz 官方说明 dismax 会自动转义特殊查询语法并使用 plain indexed search；这样可沿用默认跨字段语义，同时继续限制 200 scalars/limit10/fixed host/1rps。
- **2026-08-26 — D18:** 编辑器采用 preserve-mode 原生 CSS Grid 双工作区，不引入新设计系统。桌面左 46% 本地草稿、右 54% 联网搜索并各自滚动；<860px 单栏。候选使用语义化 `dl` 字段列表，来源不再拼进无标签句子。在线封面点击即选入草稿，成功必须显示 object-URL 预览/来源，失败必须在长列表上方可见；不新增“下载后再选择”的第二阶段，以免产生与现有 Apply 语义冲突。
- **2026-08-26 — D19:** 本地封面候选在左 pane 使用 `repeat(auto-fill, minmax(112px, 1fr))`，典型桌面宽度形成三列；button 固定 `104px + two-line label` rows 且 `overflow:hidden`，图片使用固定 block-size/`object-fit:cover`。不再依赖 replaced image intrinsic aspect ratio 参与 outer grid row sizing。
- **2026-08-26 — D20:** 免费封面主源改为 MusicBrainz Cover Art Archive release-group `front-500`，TheAudioDB 降为 fallback。CAA lookup 禁止自动 redirect并验证首个 storage location；image fetch 仅允许 HTTPS `coverartarchive.org`、`archive.org` 和 `*.archive.org`，最多 3 次 redirect，继续执行 bytes cap 与 `validate_cover`。不允许调用方 URL 或通用 redirect policy。
- **2026-08-26 — D21:** TheAudioDB `album:null` 是合法 empty response；API query 发送 bounded trimmed original artist/album，NFKC/lower/alphanumeric normalize 只用于响应后的唯一 exact comparison。Production timeout 与已验证网络基线统一为 10s connect/20s total。

## Review Notes

- [x] **2026-08-26 — M11/M12 release review Pass:** CAA lookup 只由 MBID 构造，首个 storage location 必须是 HTTPS `archive.org`/子域；fetch custom policy 同一 allowlist且最多 3 redirects，所有 bytes cap/validate。Composite tests prove CAA hit zero TADB dispatch、CAA 404→TADB null→raw multiword exact fallback、clean none 与 host-dispatched fetch。Native TENT CAA preview/Cancel passed。Version/lock/config/bundle audit is consistent at 0.4.0. No code or plan finding remains for M11/M12；tag/push task T12.3 is the only release action pending.
- [x] **2026-08-26 — Pass with Notes after theme merge `6b8a248`:** 当前 HEAD `6b8a248` 位于 M9/M10 commit `5123c6c` 之后。主题提交保留 split workspace/cover draft/result semantics，并新增 rate-limit deadline 自动清除与 `applyTheme("nord")` 时 open editor/unsaved title 保持测试。独立复验：Vite+ fmt/check/build 通过，19 files/124 tests 通过；root provider 32、desktop online 28、desktop all-targets 89、security 4、license 1、desktop clippy 和 debug `.app` bundle build 通过。`theme-contract.test.ts` 覆盖全部 9 preset 的完整 semantic tokens、action/state/focus contrast；原生 Rosé Pine Dawn 与 Tokyo Night 抽样确认 dialog、双栏、错误 alert、本地封面三列/两行 label、固定 actions 和焦点对比度正常，QA 草稿 Cancel 后将主题恢复为原 Rosé Pine Dawn 并退出。实时 MusicBrainz 在 Tokyo Night 下连续两次返回真实 `provider_unavailable`，因此本轮未宣称新主题下 live populated result-card pass；现有 M10 live result evidence与确定性 result DOM/theme contract 仍有效。无代码 finding，既有 T8.1/T5.2–T5.4 外部缺口不变。

## Completion Evidence

- M11/M12 release candidate (2026-08-26): CAA 6, TheAudioDB 13, composite free artwork 4, full provider 43; desktop online 29, all-targets 90, security 4, license 1; root all targets/features 191 plus serial watch/no-network/provider mock; Vite+ 19 files/125 plus fmt/check/build; debug no-bundle and `.app` 0.4.0 builds passed. Native `Jackie / toconoma / TENT` retrieved a real CAA front image through validated archive redirects, displayed it only in the draft, then Cancel/reopen proved no persistence. No dependency was added. Existing Vite warnings are unchanged. Implementation commit/tag/remote evidence is recorded by T12.3 after release actions.

- M10 desktop workspace (2026-08-26): targeted `rtk vp -C desktop test --run MetadataEditor.test.tsx` passed (34); full Vite+ test passed (17 files/102); Vite+ fmt/check/build passed; root provider subset passed (32), desktop online metadata (28), security policy (4), and diff check passed. Native debug `.app` light/dark wide-layout QA showed 46/54 independent panes, fixed actions, labeled 10-result list, and corrected three-column local artwork cards with contained 104px images and two-line labels. A real TheAudioDB timeout rendered the visible cover-service failure above results and inside the candidate; deterministic tests prove successful bytes create/revoke the left draft preview and remain draft-only until Save. Narrow fallback is source-locked at 860px/560px; native resize was unavailable through Computer Use and is not claimed.

- M9 regression fixes: `rtk vp -C desktop test --run MetadataEditor.test.tsx` passed (31); full `rtk vp -C desktop test` passed (17 files/99); `rtk vp -C desktop fmt . --check` and `check --no-fmt` passed. `rtk cargo test --locked metadata::provider::musicbrainz -- --test-threads=1` passed (9); full provider subset passed (32); desktop `online_metadata` subset passed (28); `rtk cargo check --locked -p unlockm-desktop`, `rtk cargo fmt --all -- --check`, and `rtk git diff --check` passed. A transport-only production-client probe changed from 3.007s `RequestTimeout` before the timeout fix to 6.888s `Ok(10)`; it did not inspect relevance, which the subsequent native review correctly reopened. One intermediate 5.797s `Unavailable` was a real provider response and not a client timeout. The temporary probe source was removed.
- M9 relevance fix: official MusicBrainz Search documentation and a live API comparison established that `dismax=true` is plain indexed search with provider-side syntax escaping. Targeted checks passed: MusicBrainz 9, all provider 32, desktop online metadata 28, security policy 4, Rust fmt and diff check. A rebuilt `target/debug/bundle/macos/UnlockM.app` native run kept artist focus for 3s and returned `Relive / toconoma / TENT / 301s` first for `relive toconoma` in about 8s; the draft was cancelled without Save.
- M9 final re-review/pass (2026-08-26): `rtk cargo fmt --all -- --check`; root `clippy --all-targets --all-features -D warnings`; root `test --all-targets --all-features` (180); `no_network_by_default` (1); `metadata_provider_mock` (1); desktop clippy; desktop all-targets (87), security (4), license (1); Vite+ fmt/check/full test (17 files/99)/build; `rtk vp -C desktop exec tauri build --debug --no-bundle`; and `rtk git diff --check` all passed. No M9 review finding remains open. Existing build warnings are unchanged and non-blocking.

- M1 automated: `rtk cargo check -p unlockm-desktop --locked` passed; `rtk cargo test -p unlockm-desktop online_metadata --locked -- --test-threads=1` passed. `Cargo.lock` now records `keyring` 4.1.6 and `getrandom` 0.3.4.
- M1 re-review evidence (reviewed/pass): `rtk cargo check -p unlockm-desktop --locked` passed; `rtk cargo test -p unlockm-desktop online_metadata --locked -- --test-threads=1` passed (8); `rtk cargo test --locked metadata::provider::spotify` passed (2); `rtk cargo test -p unlockm-desktop --test license_audit --locked` passed (1).
- M2 automated: `rtk cargo fmt --all` passed; `rtk cargo check -p unlockm-desktop --locked` passed; `rtk cargo test --locked metadata::provider::spotify` passed (3); `rtk cargo test -p unlockm-desktop online_metadata --locked -- --test-threads=1` passed (9); `rtk git diff --check` passed. Real Spotify/API, actual CDN image and platform credential tests remain M5 gaps.
- M1 second-fix validation (reviewed/pass): `rtk cargo fmt --all` passed; `rtk cargo check -p unlockm-desktop --locked` passed; `rtk cargo test -p unlockm-desktop online_metadata --locked -- --test-threads=1` passed (12); `rtk cargo test --locked metadata::provider::spotify` passed (3); `rtk git diff --check` passed.
- M1 final-gap validation (reviewed/pass): `rtk cargo fmt --all` passed; `rtk cargo test -p unlockm-desktop online_metadata::auth --locked -- --test-threads=1` passed (6); `rtk cargo test -p unlockm-desktop online_metadata --locked -- --test-threads=1` passed (14); `rtk cargo check -p unlockm-desktop --locked` passed.
- M2 second-fix validation (pending independent re-review): `rtk cargo test --locked metadata::provider::spotify -- --test-threads=1` passed (8); `rtk cargo test -p unlockm-desktop online_metadata --locked -- --test-threads=1` passed (18); `rtk cargo check -p unlockm-desktop --locked` passed; `rtk vp -C desktop check --no-fmt` passed; `rtk git diff --check` passed.
- M2 final-race validation (pending independent re-review): `rtk cargo test -p unlockm-desktop online_metadata --locked -- --test-threads=1` passed (19); `rtk cargo check -p unlockm-desktop --locked` passed.
- M2 independent review/pass: `rtk cargo test --locked metadata::provider::spotify -- --test-threads=1` passed (8); `rtk cargo test -p unlockm-desktop online_metadata --locked -- --test-threads=1` passed (19); `rtk cargo check -p unlockm-desktop --locked` passed; `rtk git diff --check` passed. Review notes accepted without blocking M3.
- M3 validation (pending independent review): `rtk vp -C desktop fmt . --check` passed; `rtk vp -C desktop check --no-fmt` passed; `rtk vp -C desktop test` passed (17 files, 75 tests); `rtk vp -C desktop build` passed with pre-existing ineffective dynamic import and >500k chunk-size warnings; `rtk git diff --check` passed.
- M4 validation (pending independent review): `rtk cargo fmt --all` passed; `rtk cargo test -p unlockm-desktop --test security_policy --locked` passed (3); `rtk cargo test -p unlockm-desktop --test license_audit --locked` passed (1); `rtk vp -C desktop test --run locales contracts` passed (2 files, 4 tests); `rtk git diff --check` passed.
- M6/M7 validation (ready for independent review): `rtk cargo fmt --all -- --check` passed; `rtk cargo test --locked metadata::provider -- --test-threads=1` passed (28); `rtk cargo test --locked -p unlockm-desktop online_metadata -- --test-threads=1` passed (23); `rtk cargo test --locked -p unlockm-desktop --test security_policy` passed (4); `rtk cargo check --locked -p unlockm-desktop` passed; `rtk vp -C desktop fmt . --check` passed; `rtk vp -C desktop check --no-fmt` passed; `rtk vp -C desktop test --run MetadataEditor.test.tsx` passed (24); `rtk git diff --check` passed.
- M6/M7 re-review/pass: M6 `rtk cargo test --locked metadata::provider::musicbrainz -- --test-threads=1` passed (9), `rtk cargo test --locked metadata::provider::theaudiodb -- --test-threads=1` passed (12 after the URL-cap minor fixture), `rtk cargo test --locked -p unlockm-desktop online_metadata -- --test-threads=1` passed (26), `rtk cargo test --locked -p unlockm-desktop --test security_policy` passed (4), fmt/diff check passed. M7 `rtk vp -C desktop test --run MetadataEditor.test.tsx` passed (28); independent re-review recorded full Vite+ suite (94), root provider (31), desktop online (26), frontend check/fmt/diff as passed.
- M8/M5 automated: `rtk cargo fmt --all -- --check`, `rtk cargo clippy --locked --all-targets --all-features -- -D warnings`, and `rtk cargo test --locked --all-targets --all-features` passed. Serial `rtk cargo test --locked --test watch_recursive -- --test-threads=1` (2), `watch_stability` (2), `no_network_by_default` (1), and `metadata_provider_mock` (1) passed. `rtk cargo clippy --locked -p unlockm-desktop --all-targets -- -D warnings`, `rtk cargo test --locked -p unlockm-desktop --all-targets` (85), `security_policy` (4), and `license_audit` (1) passed. `rtk vp -C desktop fmt . --check`, `check --no-fmt`, full `test` (17 files/94), and `build` passed (two non-blocking Vite warnings above). `rtk vp -C desktop exec tauri build --debug --no-bundle` built `target/debug/unlockm-desktop` with no Spotify client ID. `rtk git diff --check` passed.
- M4 independent final review: `rtk cargo test --locked -p unlockm-desktop --test security_policy` passed (4); `rtk cargo test --locked -p unlockm-desktop --test license_audit` passed (1); static release guard was checked with blank and configured public client IDs; `rtk git diff --check` passed.
- Final re-review: full `rtk vp -C desktop test` passed (17 files/96 tests); `rtk cargo test --locked -p unlockm-desktop online_metadata -- --test-threads=1` passed (28); `rtk cargo test --locked metadata::provider -- --test-threads=1` passed (32); `rtk cargo test --locked -p unlockm-desktop --test security_policy` passed (4); desktop clippy and `rtk git diff --check` passed.
- Native partial QA / timeout revision: merged-main debug `.app` built successfully with `RUSTC=/opt/homebrew/opt/rustup/bin/rustc rtk vp -C desktop exec tauri build --debug --bundles app` while preserving sccache. `rtk cargo test --locked metadata::provider::musicbrainz -- --test-threads=1` passed (9) after the 20s production-timeout assertion. Manual no-account UI checks listed in Progress passed only through local/draft/save/restart/secret-audit boundaries; broad/narrow real MusicBrainz searches timed out after both the 8s and 20s builds. Elevated curl to the same endpoint returned HTTP 200 in 11.150974s. T8.1 remains incomplete because no real candidate/TheAudioDB cover could be exercised; T5.2–T5.4 remain external gaps.
- 完成时必须记录：最终 commit(s)、依赖/version/license 变化、全部自动命令结果、macOS/Windows secure-store QA、真实 Spotify configured flow、SQLite/log/IPC secret audit、policy/quota disposition 和独立 review 结论。
