# Desktop History, Player, Metadata, and Display UX Improvements

## Status

In Review

Last updated: 2026-08-25

## Objective

改进 UnlockM 桌面端的长期音乐库使用体验：允许为历史转换任务命名并基于原配置原位重新扫描执行；让标准音频、格式与码率事实正确进入任务详情；把播放能力提升为跨主页/详情页和分页持续存在、带安全默认音量与单曲循环的全局播放器；提供安全、原子、可恢复且不溢出应用窗口的输出音乐元数据编辑；补齐长列表导航、紧凑筛选、单行媒体资料库布局、macOS/Windows 界面密度与多配色偏好；并针对 v0.3.1 的实际界面反馈，精修 sticky 主页入口、多语言筛选宽度、可见结果序号、可拖动标题/专辑列和播放器收展控件几何。

完成后，用户应能在不接触文件路径或数据库细节的前提下管理历史任务和输出音乐；CLI 的转换语义、源文件保留、原子发布、路径 containment、碰撞处理和有界处理等核心不变量保持不变。

## Scope

- 历史任务新增可持久化名称、重命名入口和“重新执行”入口。
- 重跑从历史记录读取原输入、输出和设置，重新扫描当前文件树并原位替换同一 run ID 的执行事实与歌曲明细；显示名保持不变、任务更新时间刷新、历史列表记录数不增加。
- 主页与详情页的长列表提供滚动后出现的固定“返回顶部”按钮；详情页固定左上角“返回主页”入口使用有背景、边框和阴影的紧凑按钮，sticky 后与 68px 顶栏之间保持明确视觉间隔，不能贴边或与内容文字混在一起。
- 将唯一的 `<audio>` 引擎和播放状态提升到页面切换之外，增加左下角固定、可收起的播放器，展示圆形封面、歌曲名、歌手、专辑、进度和时长，并提供音量滑杆、静音/恢复、低于 100% 的安全默认音量和单曲循环。
- 详情歌曲行改为紧凑单行媒体资料库布局：序号/悬停播放、封面、标题+歌手两行、独立专辑列、格式列、右对齐大小+码率列与操作列；序号按当前查询结果顺序连续显示，标题与专辑共同占满固定列以外的空间并可由用户拖动分割比例；整行的非操作区域可点击播放，歌手/专辑只有实际文本区域可点击并应用对应筛选。
- 新增单曲元数据编辑弹窗，可编辑/清空标题、歌手和专辑，可上传不超过 10 MiB 的受支持图片，也可选择本地历史中已有的封面；相同专辑封面优先展示。弹窗和候选区域必须受当前应用 viewport 约束并提供内部滚动，底部操作始终可达。
- 在任务详情中展示每首音乐的码率；标准音频即使走 passthrough copy，也作为音乐而不是 companion 显示、筛选和播放。
- 修复带较大 ID3/APE 前置标签的 MP3 只读 8 KiB 后被显示为“未知格式”的问题，并修复可安全识别的旧历史事实。
- 搜索、歌手、专辑、结果筛选各自拥有输入框内部的条件式小型清空按钮；搜索/歌手/专辑默认保持紧凑坍缩，但坍缩宽度仍须完整显示中/英/日当前占位文案，仅在 focus 或已有值时平滑展开，结果和排序控件按内容宽度收紧。
- 独立播放器展开时彻底隐藏封面 hover 播放层；折叠时封面与播放层使用同一圆形尺寸并完全重合。收展入口使用 Phosphor 方向图标和专用居中 icon-button 几何，不显示形似原始 `<>` 字符的未对齐控件。
- 折叠播放器的圆形封面与展开入口在共享 fixed geometry 中保留约 8px 的单一、不会双算的可见间隔。
- 歌曲列表只在与行列严格对齐的紧凑 column header 中提供一个可访问的标题/专辑拖动 handle；歌曲行保留无视觉线、无交互/tab stop 的同一 spacer track，并显示歌曲/专辑/格式/大小与码率等本地化列标题。
- 保留亮/暗模式，并新增默认蓝、翡翠绿、紫罗兰、暖橙四种配色风格，持久化到 Rust preferences。
- 为 macOS 和 Windows 建立共用的界面密度基线：同步增大字体、图标、控件和列表信息密度，收紧当前过大的卡片空白、间距与 padding；再使用 Tauri 原生窗口缩放率和缩放变化事件做有上限的校准，避免整页 `zoom`/transform 造成模糊或布局溢出。
- 更新中/英/日三套文案、Rust/TypeScript IPC 契约、自动化测试、手工 QA 清单及 `ARCHITECTURE.md`。

## Non-Goals

- 不加入音频转码、均衡器、播放队列、上一首/下一首、系统媒体键或云端播放同步；音频控制范围仅包含当前单曲循环、播放/暂停、seek、音量和静音。
- 不修改源音乐文件；元数据编辑只针对已持久化、已验证的成功输出文件。
- 不加入外部网络封面搜索或新的元数据服务；“搜索已有封面”只查询本地 SQLite 历史中的去重 artwork。
- 不允许前端传入任意输出路径，不开放通用文件系统、asset protocol、shell 或进程能力；自定义封面以受限字节 payload 传给 Rust。
- 不改变 CLI 的复制开关、覆盖策略、输出布局、watch 模式或命令行界面；共享 runner 只补充媒体分类/事实，不改变发布结果。
- 不自动覆盖重跑时已经存在的输出文件。重跑沿用历史快照中的 `overwrite = false`，已有输出记为 skipped，新下载的文件正常进入同一规划管线；“原位替换”只针对 SQLite 中该任务的执行事实和条目。
- 不批量编辑整张专辑的标签；本计划只提供单曲编辑和相同专辑封面快捷选择。
- 不修改 SQLite 查询、持久化 ordinal 或后端分页协议来实现界面序号；序号仅由当前筛选/排序结果中的分页位置派生，不写回持久化数据。
- 不把标题/专辑分割比例写入 Rust preferences 或 SQLite；本轮只在当前详情会话内保持比例，重新打开应用使用经过验收的默认比例。
- 不为窄于现有 820px 断点的布局强行保留专辑列或拖动手柄；该断点继续优先保证标题、播放和操作按钮可达。

## Current Context

### 2026-08-23 原始基线（M1–M8 已实现，保留作决策背景）

- `desktop/src/App.tsx` 同时拥有主页/详情页切换、历史列表和转换启动状态。任务显示名目前直接取 `inputPath` basename；历史行只有详情、显示输出目录和删除操作。
- `desktop/src/features/history/HistoryDetail.tsx` 当前拥有唯一 `<audio>`、active item、播放时间和进度状态。组件卸载、分页或筛选导致活动条目离开当前 25 条结果时会执行 `stopAudio()`，因此返回主页或翻页会停止播放。
- `HistoryDetail.tsx` 的每个歌曲行包含播放按钮、range 进度和时间；筛选工具栏只有一个 `resetFilters()`，会一次清空搜索/歌手/专辑/结果并重置排序。
- `desktop/src-tauri/src/history/mod.rs` 使用 SQLite schema v2。`runs` 没有任务名；`items` 有输出大小/格式和标题/歌手/专辑/封面，但没有码率、媒体探测版本或元数据编辑恢复标记。`Preferences` 只有 locale、light/dark 和默认转换设置，没有播放器音量或 palette。
- `desktop/src-tauri/src/commands/mod.rs` 的 `start_conversion_blocking` 已经是桌面转换的唯一入口并受 `AppState` one-active-run 约束；重新执行必须复用该配置构建、扫描、observer、持久化和完成流程，不能另造一条 runner。
- `src/app/runner.rs` 的 `planned_item_kind()` 把所有 `PlannedAction::Copy` 都标记为 `Companion`。`src/app/job.rs` 只为 decrypt action 生成 `MusicSnapshot`，所以正常 MP3/FLAC 等 passthrough 输出不会进入 `query_music_items()`。
- `src/formats.rs::prepare()`、`src/app/job.rs::output_facts()`、`desktop/src-tauri/src/history/mod.rs::probe_output_facts()` 和 `desktop/src-tauri/src/audio_protocol.rs` 都只读取最多 8192 字节再调用 `formats::extension()`。`src/crypto/audio.rs` 能解析 ID3/APE 声明长度，但当音频帧位于该窗口之外时返回 `NeedMoreHeader`，这解释了有效 MP3 显示“未知格式”的现象。
- `src/metadata/tags.rs` 已通过 Lofty/专用 FLAC writer 提供内容探测、10 MiB 封面验证、候选文件写入、回读验证和 `AtomicOutput` 发布，可作为手工元数据编辑的共享安全边界；目前写入模型没有显式“清空字段”语义。
- `desktop/src-tauri/src/audio_protocol.rs` 仅接受 item ID，并把成功的 `converted`/`skipped` music item 映射到验证后的输出文件；标准音频改为 `copied` music 后需要显式纳入该成功集合。
- `desktop/src/styles/tokens.css` 只有一套蓝色 token 的 light/dark 变体。多个详情字号低至 `0.58rem`–`0.72rem`；`App.css` 中还有较高的 drop-field、较大的 section gap 和 padding，导致 macOS/Windows 上同时呈现“元素偏小、空白偏多”。
- 锁定的 `@tauri-apps/api` 已提供 `getCurrentWindow().scaleFactor()` 和 `onScaleChanged()`；当前 core window default ACL 已含只读 scale-factor 能力，无需增加广泛窗口权限。原生 factor 不能直接作为 CSS 倍率，否则 Retina/WebView2 已应用的 DPI 会被重复放大。
- `docs/plans/2026-08-21-tauri-desktop-app.md` 已为 `Done`，记录了现有桌面架构、ID-only 播放协议和 macOS/Windows 发布证据。本计划是新的增量工作，不重开旧计划。
- `v0.3.0` 已提交为 `83b503bf5433f9f47711f14e79ccd15490d56b07`、打 annotated `v0.3.0` tag、推送 `origin`/`github`，本轮修订从该干净工作树继续；本地 `/Applications/UnlockM.app` 是该提交构建的 arm64 0.3.0。
- 当前 `rerun_conversion()` 在 `desktop/src-tauri/src/commands/mod.rs` 读取旧快照后调用普通 `start_conversion_blocking()`，后者经 `AppState::begin_run()` 插入新 `runs` 行，最后再复制名称；这与“同一任务原位刷新”的新产品语义冲突。
- 当前 `AppHeader.tsx` 只为已选 palette option 渲染 `Check`，导致同一文字列在选中/未选中时改变可用宽度；菜单应永久预留固定尾列并只切换可见性。
- 当前详情返回入口的 `.back-button` 是透明 sticky 文字，滚动到歌曲内容上时没有独立表面；筛选 toolbar 的 CSS grid 让三项文本筛选扩张占满行，非搜索清空按钮也没有输入框内定位。
- 当前歌曲行把标题、歌手、专辑堆成三行，并单独常显播放按钮；需要改为资料库式列布局，同时保证编辑/显示目录/默认打开按钮不会被整行播放点击吞掉。
- 当前 metadata modal 只有固定 `max-width`，没有 viewport 高度上限或滚动容器；12 个候选加表单已足以在较矮窗口中把操作按钮挤出可见区域。
- 当前 `PlayerArtwork` 在 `artworkId` 变为 `null` 时提前 return 而不清除旧 object URL，封面动画只由“展开”状态控制；播放器 DOM 在收起时移除封面，播放按钮位于整条布局最左而不是进度条左侧，并使用加减号作为收展图标。

### 2026-08-25 v0.3.1 当前修复基线

- v0.3.1 当前基线是干净提交 `d0487d03e786967127aa35706522105d9c44dda4`，本地 annotated `v0.3.1` tag 未推送；M7–M8 的代码 findings 已闭环，但 Windows T6.3 原生 DPI/runtime/跨显示器 QA 仍开放。本轮是对同一活动计划的后续 UI 精修，因此重开为 `Ready`，不另建重复计划。
- 2026-08-25 的四张 PixPin 截图分别位于 `/Users/peilin/Library/Application Support/PixPin/Temp/PixPin_2026-08-25_16-58-52.png`、`PixPin_2026-08-25_17-00-36.png`、`PixPin_2026-08-25_17-04-28.png` 和 `PixPin_2026-08-25_17-11-44.png`；它们显示 sticky 返回入口贴住顶栏、英文搜索占位被截断、筛选结果序号跳号/右侧列距失衡，以及播放器封面覆盖层/收展控件的几何问题。
- `desktop/src/App.css` 把 `.topbar-inner` 高度硬编码为 `68px`，`desktop/src/styles/history.css` 又把 `.back-button` 的 `top` 写为同一个 `68px`，sticky 生效时没有额外间距；`desktop/src/i18n/locales/*` 的 `app.back` 仍是“返回转换列表 / Back to conversions / 変換一覧に戻る”。
- 同一 CSS 将普通 compact filter 的坍缩宽度写死为 `112px`、搜索写死为 `144px`。搜索还要容纳 18px 图标、gap 和 padding，导致 `Search songs` 等文案在未 focus 时被截断；歌手/专辑的最长日文占位也没有原生窗口验收保障。
- `HistoryDetail.tsx` 当前用 `item.ordinal + 1` 渲染序号；该 ordinal 是持久化的原扫描顺序，所以筛选或非 ordinal 排序后会出现 `02, 03, 05, 07`。`.music-item` 的 `1.25fr / 0.8fr / 68px / minmax(..., 0.45fr)` 轨道让大小/码率列继续吞掉剩余空间，形成格式列后的异常空白。
- 歌手和专辑都是 `<button>` grid item；缺少 `justify-self: start`/intrinsic inline sizing 时会沿整列 stretch，视觉文字右侧的空白也成为筛选 hit area，与整行点击播放发生误操作。
- `PersistentPlayer.tsx` 已使用 Phosphor `CaretLeft/CaretRight` SVG，但切换按钮没有专用 class/居中尺寸，视觉仍像未居中的 `<`/`>` 字符。封面是 `42px`，通用 `.persistent-player .play-button` 把覆盖按钮固定成 `40px`；展开态仅依靠 `.collapsed-play { opacity: 0 }` 和 `disabled`，不足以建立“不可见且不可命中”的强状态契约。

## Design and Decisions

### 1. 任务名称与重新执行语义

- SQLite schema 升级时给 `runs` 增加 nullable `display_name`。API 始终返回非空 `displayName`：新任务默认使用输入 basename，旧记录在读取时使用相同 fallback；用户重命名后存储 trim 后的 1–120 字符名称。
- 重命名只在详情头提供。保存调用 ID-only `rename_run(run_id, display_name)`，只允许 terminal run，返回更新后的 `RunSummary`；不修改输入路径、输出路径或文件。
- `rerun_conversion(run_id, on_event)` 从 Rust 历史读取快照并重新验证输入/输出，不信任前端重传路径或设置。它与新任务复用同一个 conversion/observer/coordinator pipeline，但通过新的 state-owned `begin_rerun(run_id)` 进入原位替换分支。
- `begin_rerun` 在 one-active-run 锁下事务性确认目标仍为 terminal，保留 run ID、显示名与请求快照，删除旧 item 明细并清理已失去引用的 artwork，重置计数、代表封面、terminal error/status/timestamps/app version，再把同一 ID 设为 active。不存在、非 terminal、删除/清空/其他转换竞态必须稳定失败，不能创建替代行。
- 重跑会重新枚举源文件，因而可发现后来下载的音乐；旧输出仍按 `overwrite = false` skipped。完成、失败或取消都覆盖同一任务的新执行事实，历史总数不变；执行时间戳刷新，但同一 run ID 在现有 ID-keyset 历史分页中的相对位置保持稳定，不为本需求重写游标协议。

### 2. 统一媒体分类、格式与码率事实

- 给 preflight 结果保留“标准音频 / 加密音乐 / companion”媒体类别，不能再用 `Copy` 推断 `Companion`。标准音频 copy 或因 copy 关闭而 skipped 时均记录为 `Music`；未知非音频才是 `Companion`。
- 对成功的标准音频 copy 生成与 decrypt 相同的有界 `MusicSnapshot`，从实际输出（同路径 no-op 时从验证后的源）读取标题、歌手、专辑、时长、封面、输出大小、格式和码率。
- `MusicSnapshot`、`PersistedItemInput`、`HistoryItem` 和 TypeScript contracts 增加 nullable `bitrateKbps`。码率优先使用 Lofty audio properties；无法可靠取得时保持未知，不根据扩展名伪造。
- schema 新增 `items.bitrate_kbps` 和 `items.media_probe_version`。新记录写当前版本；旧 copy/skipped 记录在详情读取期间每次最多处理固定批量，内容探测后更新 `item_kind`、格式、码率及媒体快照。响应暴露 repair pending，前端以空闲刷新继续推进，避免一次性扫描整个历史或阻塞无限列表。
- `copied` 加入 music outcome filter、播放/显示/元数据编辑的成功集合。历史 run 的 `musicCount`、facets 和详情总数在有界修复完成后与实际音乐一致。

### 3. MP3 内容探测

- 在 `src/formats/audio.rs` 集中提供 path-based bounded probe。它读取最小头、用 checked offset 跳过最多固定层数的 ID3/APE 前置标签，再从真实音频 payload 读取小窗口验证连续 MP3 frame；标签声明长度只用于 seek 且必须落在文件长度内，绝不按标签大小分配内存。
- 现有字节切片探测仍服务解密 prefix。凡已有真实路径的标准音频分类、发布后 facts、历史修复和 audio protocol MIME 校验统一使用 path probe，消除四处各读 8 KiB 的分叉。
- 添加普通 MP3、无 ID3 MP3、超过 8 KiB/128 KiB 的合成 ID3 前缀、截断/恶意长度和非音频伪装 `.mp3` 回归。只有内容确认后才显示 MP3；无法确认仍显示未知，不以文件扩展名兜底。

### 4. 持久播放器与导航

- 在 `desktop/src/features/playback/` 新建 provider/controller 和 `PersistentPlayer`。provider 挂在 `App` 的页面 `AnimatePresence` 外，持有唯一 `<audio>`、活动 `PlaybackTrack`、播放/暂停、currentTime/duration、seek、错误和 collapsed 状态。
- `HistoryDetail` 的行按钮只调用 provider 的 `playOrToggle(item)`，不再拥有 audio ref，也不因翻页、筛选或页面卸载停止活动音乐。播放器只在用户换歌、主动暂停/结束、记录被删除、输出失效或应用退出时释放 source。
- 左下固定播放器展开时显示圆形封面、标题、歌手、专辑、位于进度条左侧的播放按钮、进度 range、时间、音量 range 和静音/恢复按钮；收起后默认只突出圆形封面，hover/focus-within 时才覆盖显示播放/暂停按钮。播放时才旋转封面，暂停/停止立即保持当前角度；`prefers-reduced-motion` 下始终静态。
- `<audio>` 使用 native loop 进行当前单曲循环；切换到 `artworkId = null` 或封面加载失败时立即撤销/清除旧 object URL 并显示当前歌曲占位图，绝不沿用上一首封面。
- 收展控件使用语义明确的左右箭头，布局宽度/内容显隐以 Motion layout 或可中断 CSS transition 平滑过渡；reduced motion 下禁用位移动画。窄窗口、safe-area、错误 banner、右下返回顶部按钮和右对齐分页不得互相遮挡。
- `Preferences` 增加 `playerVolume`（0.0–1.0，旧数据/缺失/非法值默认 0.6）。controller 必须在第一次 `play()` 前把 60% 或持久化值赋给 `audio.volume`，绝不依赖浏览器 100% 默认；用户结束拖动或键盘调整后节流持久化。静音按钮在会话内记住最近的正音量，恢复时回到该值或 0.6；显式把滑杆调到 0 可以作为持久静音。
- 当前曲目被元数据编辑后刷新播放器快照/封面；编辑当前曲目前先暂停并在成功后从新文件重新装载，避免 Windows 文件占用冲突。
- 新建共享滚动控制：超过阈值后右下显示固定返回顶部按钮，按 reduced-motion 选择 smooth/instant；详情页左上固定主页按钮调用现有 `onBack`。主页/详情切换后把新页面定位到顶部，但不触碰播放器。

### 5. 元数据编辑与本地封面复用

- 新增 ID-only `update_run_item_metadata`。Rust 先确认 item 是 terminal run 中带成功输出的 music、输出仍为非 symlink regular file，并在无 active conversion 时取得历史 mutation gate；前端永远不提交目标音频路径。
- 编辑 payload 对标题/歌手/专辑使用明确的 set/clear 语义。trim 后每字段最多 512 字符；空输入表示清空对应 tag。封面动作限定为 keep、引用现有 `artworkId` 或上传受支持的图片 bytes；前后端都执行 10 MiB 上限，Rust 仍以 `tags::validate_cover` 为权威。
- 扩展 `src/metadata/tags.rs` 的候选写入，使 set/clear 三字段和替换 front cover 都被回读验证。候选始终位于输出旁，验证成功后才通过 `AtomicOutput` 替换；源文件和已有输出在失败时不变。
- schema 增加 `items.metadata_edit_pending`。命令在发布前提交 pending 标记，发布后从真实文件重读标签/封面/格式/大小/码率并事务性更新 item、去重 artwork、必要时更新 run representative artwork，然后清 pending。若进程在两阶段之间退出，下一次 bounded detail repair 以磁盘真实内容恢复历史并清理标记。
- `list_artwork_candidates(item_id, query, cursor, limit)` 只返回本地已引用 artwork 的轻量 ID、专辑、歌手、代表歌曲信息。当前 item 的标准化专辑完全匹配项置顶，其后是 bounded 文本搜索结果，并按 artwork SHA 去重；图片 bytes 仍通过现有 lazy `get_artwork` 获取。
- 编辑弹窗支持键盘焦点管理、Escape/取消、保存 busy/error、上传预览/object URL 清理，以及同专辑快捷候选和搜索。成功后原位更新当前页、facets 和全局播放器，不要求整页重启。

### 6. 独立筛选、配色和跨平台界面密度

- 搜索、歌手、专辑、结果四个控件各自只在有值时在输入/选择框内部尾侧显示可访问的圆形 `×`，控件为其永久预留尾部 padding，清空后回到第 1 页。搜索清空同步取消 debounce 值；排序保持当前选择并与筛选清空解耦。
- 搜索、歌手、专辑使用同一 compact-expandable 控件：空值且未 focus 时按图标/短标签宽度坍缩，pointer 或键盘 focus 后用 `inline-size`/grid track 动画展开，已有值时保持展开；结果和排序按内容宽度布局，不用 `1fr` 吞掉剩余空间。`prefers-reduced-motion` 下只切换布局不动画。
- `Preferences` 增加带 serde default 的 `palette`：`blue`（旧数据默认）、`emerald`、`violet`、`amber`。`AppHeader` 增加带色样、选中态和完整键盘导航的 palette menu；`applyTheme` 同时设置 `data-theme`、`data-palette` 和 `color-scheme`。
- 每套 palette 只覆盖语义 accent/focus/selected token，并为 light/dark 提供可读对比；危险、成功、警告语义不被装饰色混淆。Rust preference 是权威，localStorage 只用于首帧防闪烁。
- 新建 `desktop/src/lib/display-scale.ts`：在 macOS 和 Windows 读取原生 scale factor 并监听 monitor scale change，把经过平台归一化和 clamp 的温和比例写入 CSS custom property。不得把 Retina 的 `2.0` 或 Windows DPI factor 原样乘到页面上；同一个纯函数必须能用平台/factor fixture 单元测试。
- `tokens.css` 增加字体、图标、控制高度、列表行、间距和内容密度 token。把低于 `0.75rem` 的正文/事实/时间字号提升到可读下限，增大主要按钮/输入框/图标/封面，同时缩小 composer 空面板的最小高度、section 大间距和无信息 padding，让常见 1080×760 窗口在不拥挤的前提下承载更多有效内容。
- macOS 与 Windows 使用同一视觉基线和响应式断点，只允许 native factor 校准存在必要的平台差异。验收覆盖 macOS 默认/更多空间/较大文字和可用的内外接显示器，以及 Windows 100%、125%、150%、200% 与跨 DPI 显示器移动；任何模式下都不能有工具栏截断、播放器/固定按钮遮挡或模糊 transform。

### 7. 安全、兼容和状态同步

- 所有新 native action 接受 run/item/artwork ID 或受限字节，不接受输出路径。`audio_protocol`、reveal/open 和 metadata edit 共用成功 music output 的验证规则并包含 `copied`。
- 删除正在播放的 run 前由 UI 停止播放器；Rust 删除仍只删记录。metadata edit/rename/rerun 的 busy/terminal 条件由后端检查，不能只依赖按钮 disabled。
- schema migration 必须从 v1、v2 和当前版本重复打开均幂等；旧 preferences 缺 palette 时默认 blue、缺/坏 `playerVolume` 时默认并 clamp 为 0.6，旧 runs 缺名称时有确定 fallback，旧 items 通过 `media_probe_version` 有界修复。
- `ARCHITECTURE.md` 更新桌面执行流、全局播放器所有权、可编辑输出元数据两阶段恢复、标准音频分类和统一 path probe；继续明确 source retention、atomic publication 与 ID-only filesystem boundary。

### 8. 媒体资料库式详情布局与弹窗约束

- 详情歌曲列表采用稳定 CSS grid 列：`序号/hover play`、`封面`、`minmax 标题+歌手`、`minmax 专辑`、`格式`、`右对齐大小+码率`、`操作`。桌面宽度下标题/歌手严格两行，专辑不再塞入标题块；窄窗口通过具名 grid area 折行，但不恢复三行元信息堆叠。
- 行 hover/当前播放态以整行 surface、边框/阴影和 accent index 强调。序号位置默认显示补零序号，hover/focus-within/active 时替换为播放或暂停图标；该按钮仍是键盘和屏幕阅读器的主要播放入口。鼠标点击行内非交互区域也调用同一 `toggle(item)`，所有歌手/专辑/编辑/reveal/open 控件显式阻止冒泡。
- 歌手与专辑使用低装饰的文本按钮，点击后只设置对应筛选、清空其它无关状态不在本次范围，并回到第 1 页；空值继续显示不可点击的 unknown 文案。
- metadata dialog 使用 `max-block-size: calc(100dvh - safe-area - margin)`；标题和底部 actions 固定在 dialog 内，表单主体与候选 grid 在中间滚动。候选区有可见、可键盘滚动的 bounded region，确保 920×600 等矮窗口中 Save/Cancel 始终可达。
- 详情分页靠容器右侧对齐，并为 safe-area/播放器留出最小间距；在窄窗口播放器接近全宽时，分页可换行到独立行但不能位于播放器下方不可点击。

### 9. Sticky 主页入口与本地化 compact filter 宽度

- 以共享 `--topbar-height`（或同等单一 token）驱动 `.topbar-inner` 高度和 `.detail-back-pill` 的 sticky offset；返回按钮的 `top` 为顶栏高度加一个稳定 spacing token，在 920×600、1080×760、native density 调整和 safe-area 下都保留约 8–12px 的清晰间隔。
- 将可见文案改为 `返回主页 / Home / ホーム`，使用明确的 `app.backHome`（或等价专用 key）避免继续把主页导航叫作“转换列表”。箭头图标、按钮语义、focus ring 和现有 `onBack`/scroll reset 行为保持。
- compact filter 的“坍缩”表示不占满剩余行，而不是裁掉自身提示。搜索、歌手、专辑分别设置能容纳当前支持语言最长占位、图标、clear-slot 和 padding 的 collapsed minimum；focus/有值后仍使用 clamp 展开，窄窗口允许整控件换行而不压缩内部文字。
- CSS 不依赖实验性的 `field-sizing: content` 或运行时文字测量；使用可审查的 control-size variables/min-inline-size，并在中/英/日原生 WebView 下逐项确认。`prefers-reduced-motion` 仍关闭宽度动画。

### 10. 当前结果序号与可拖动标题/专辑分栏

- 展示序号由当前筛选/排序结果中的分页位置派生：`(page - 1) * pageSize + visibleIndex + 1`。完整结果从 `01` 开始，翻页后连续编号；例如 `pageSize=10` 时第二页从 `11` 开始。持久化 `item.ordinal` 仍只用于后端原始顺序排序，不改 schema/DTO，也不保存展示序号。
- 桌面行把标题/歌手与专辑放入同一个“可分割的剩余空间”区域；格式、大小+码率和三项 actions 使用所有行共享的稳定 fixed CSS token tracks 与同一 column gap，不允许 facts 轨道继续按 `fr` 扩张或因单行事实文本改变。长标题/专辑使用 ellipsis，右侧信息和操作不漂移。
- `HistoryDetail` 维护 session-only `titleAlbumRatio`，默认采用接近当前视觉的标题偏宽比例。歌曲列表 header 与各行使用相同 leading/flexible/trailing CSS grid tracks；header 是唯一渲染实际 line/drag handle 的位置，歌曲行只保留同宽、`aria-hidden` 的视觉空 spacer。因此任一 ratio 变化仍让全部行对齐，但不会重复出现竖线或 tab stop。pointer drag 使用 pointer capture 并把比例 clamp 在可读范围（建议标题占可变区 35%–75%），不会触发行播放。窗口 resize 后按比例重新布局，不存入 preferences。
- header separator 使用 `role="separator"`、`aria-orientation="vertical"`、本地化 label/`aria-valuenow`，左右方向键以固定步长调整，Home/End 到达安全边界，focus ring 可见；820px 以下专辑列隐藏时 header、handle 和 tab stop 同步移除。header 至少以中/英/日显示歌曲、专辑、格式、大小与码率列；操作列可保持无文字标题。
- 歌手/专辑按钮使用 intrinsic/fit-content hit area、`justify-self: start` 和 `max-inline-size: 100%`；可见文字被 ellipsis 时仍保留完整 accessible name/title。只有实际文字（加必要 focus padding）触发筛选，列内其余空白继续走整行播放。

### 11. 播放器覆盖层、圆形几何与收展图标

- 定义一个共享 `--player-disc-size`；`.collapsed-artwork`、真实封面、占位封面和 `.collapsed-play` 都使用同一 block/inline size、`border-radius: 50%`、`box-sizing: border-box` 和居中 grid，移除通用 40px play-button 对该覆盖层的尺寸覆盖。
- 展开态对 `.collapsed-play` 同时应用 `opacity: 0`、`visibility: hidden`、`pointer-events: none`，并保持 disabled/不可聚焦；只有 `.is-collapsed:hover` 或 `.is-collapsed:focus-within` 才切换为可见。这样过渡仍平滑，但展开态不会留下淡色圆形或合成层残影。
- 收展按钮使用 `ArrowLineLeft/ArrowLineRight`（或经原生 QA 确认等价的 Phosphor 方向 icon），配专用 `.player-collapse-toggle`：固定方形 hit target、`display: grid; place-items: center`、零 line-height 和一致 hover/focus surface。expanded 表示向左收起，collapsed 表示向右展开；不渲染文本尖括号。
- 保留现有 mounted expanded-content 动画、`inert`、native loop、音量、旋转暂停角度和 reduced-motion 行为；本轮只修视觉/命中几何，不改变 PlaybackProvider 状态机。

## Milestones

### M1 — 媒体事实和历史迁移基础

- Goal: 标准音乐、MP3 格式和码率在 core observer、SQLite 和详情 DTO 中具有一致事实。
- Dependencies: 无。
- Acceptance criteria:
  - 标准 MP3/FLAC passthrough 的 terminal row 为 `music`，copied/skipped 行出现在详情并可作为成功输出使用。
  - 大 ID3 MP3 从路径探测为 `mp3`，伪装/截断文件不误报。
  - 新旧行都有 nullable bitrate；旧 facts 修复按固定批量推进，不一次加载整个 run。
- Validation:
  - `rtk cargo test --locked --all-targets --all-features`
  - `rtk cargo test --locked -p unlockm-desktop --all-targets`
  - 用正常 MP3、超大 ID3 MP3、FLAC 和非音频 companion 目录做一次桌面转换，核对数量、格式、码率和 copied outcome。

### M2 — 可命名、可重新执行的历史任务

- Goal: 任务名可编辑，历史任务可安全复用快照并重新扫描。M2 记录 v0.3.0 已交付基线；历史记录保留语义已由 D20/M7 明确取代。
- Dependencies: M1 schema/DTO migration。
- Acceptance criteria:
  - 新旧 run 都显示稳定名称；重命名在重启后保留。
  - v0.3.0 基线曾以新 run 保留旧 run；该条不再是当前验收目标，M7 要求 same-ID 原位替换、总数不变并保留名称/请求快照。
  - busy、缺失输入、失效输出和非 terminal run 返回稳定错误，不能绕过 one-active-run。
- Validation:
  - Rust history/command/state tests 覆盖 migration、rename、rerun、busy 和失效路径。
  - Frontend tests 覆盖行/详情按钮、inline/modal rename、事件接续和按钮禁用。
  - 手工在完成任务源目录新增一首歌后重跑，确认只新增输出且两条历史均可打开。

### M3 — 跨页面播放器与长列表导航

- Goal: 播放在详情分页、返回主页和再进入详情时不中断，并提供固定导航控件。
- Dependencies: M1 的成功 music/output facts。
- Acceptance criteria:
  - 全应用只有一个 audio engine；路由、分页和筛选不销毁它。
  - 左下播放器可展开/收起、seek、显示完整曲目信息，播放封面旋转且 reduced motion 静止。
  - 首次播放在调用 `play()` 前应用 60% 默认音量；滑杆、静音/恢复与持久化音量在页面切换和应用重启后行为确定，永不意外回到 100%。
  - 歌曲行没有进度条；固定主页/返回顶部按钮键盘可用、不会遮挡内容。
- Validation:
  - Vite+ tests 覆盖单例播放、页面卸载保持、换歌、seek、60% 默认音量、音量持久化、静音/恢复、collapse、错误、删除和 reduced motion。
  - macOS 和 Windows 原生 WebView 分别验证 range playback、跨页连续播放和 monitor/窗口尺寸响应。

### M4 — 原子元数据编辑与封面候选

- Goal: 用户可安全编辑输出标签并复用本地封面，失败或崩溃不损坏音频/历史一致性。
- Dependencies: M1 facts/schema；M3 player pause/reload contract。
- Acceptance criteria:
  - MP3/FLAC 标题、歌手、专辑 set/clear 及 cover replace 均回读一致，音频 payload 保持有效，源 SHA 不变。
  - 超限/坏图片、失效 ID、symlink 替换、active conversion 和 tag writer failure 都被拒绝且不发布候选。
  - 同专辑封面置顶、搜索 bounded/去重/lazy；pending edit 在模拟中断后由详情修复。
- Validation:
  - Core tag roundtrip + desktop command/history recovery tests。
  - Frontend modal accessibility、候选搜索、上传、失败重试和播放器刷新 tests。
  - 手工编辑当前播放 MP3/FLAC，重启应用并用外部播放器/标签读取器确认结果。

### M5 — 筛选、配色与 macOS/Windows 界面密度

- Goal: 高频操作更精确，界面提供持久配色，并在 macOS/Windows 上以更大的字体和控件、更少的无效留白清晰呈现。
- Dependencies: M1 DTO；可与 M2–M4 的后半段并行实现，但合并前统一 UI tokens。
- Acceptance criteria:
  - 四个筛选各自清空且不影响其他筛选/排序。
  - 四 palette × light/dark 均可选、重启保持、菜单键盘可达且关键状态对比清晰。
  - macOS/Windows 默认窗口下字体、图标、按钮和输入框明显大于当前基线，composer/section 空白减少；长路径、长标题和窄窗口仍不裁切。
  - macOS 显示缩放模式及 Windows 100/125/150/200% 和 scale-change 下布局稳定，native factor 不造成 Retina/WebView2 二次放大。
- Validation:
  - Preferences/theme/display-scale 单元测试和 App/HistoryDetail 交互测试。
  - `rtk vp -C desktop check --no-fmt && rtk vp -C desktop test && rtk vp -C desktop build`（实施时分开执行并逐项记录结果）。
  - macOS 默认/更多空间/较大文字与可用显示器组合，以及 Windows 真机的 DPI matrix；两端都完成 light/dark/palette 和常用窗口尺寸回归。

### M6 — 集成、架构文档与跨平台交付门禁

- Goal: 本计划的全部桌面体验改动作为一个可发布增量通过完整质量、安全和人工验收。
- Dependencies: M1–M5。
- Acceptance criteria:
  - 三语言语义 key 完全一致，空/加载/错误/长文本/窄窗口状态完整。
  - CSP/ACL 未增加 generic filesystem/asset/shell/process 权限；新命令全部 ID/bytes bounded。
  - `ARCHITECTURE.md` 与最终执行流、迁移版本、播放器和 metadata mutation 一致。
  - macOS/Windows no-bundle build 通过，两端界面密度、音量和原生播放/编辑有实际证据或明确记录验证缺口。
- Validation:
  - 执行 `Validation Strategy` 全部命令。
  - 完成主页、详情、重跑、重命名、滚动、播放/音量、编辑、筛选、8 个主题组合和 macOS/Windows 显示缩放的手工矩阵。

### M7 — 原位重跑与紧凑媒体资料库详情

- Goal: 修正 v0.3.0 实际使用中暴露的任务语义和详情信息布局问题，保持任务数量稳定并显著提高歌曲列表可读性。
- Dependencies: 已完成的 M1–M6；重跑 reset 必须复用现有 one-active-run/history mutation 排他边界。
- Acceptance criteria:
  - 对 terminal run 重跑后返回相同 run ID，任务总数不变、显示名保留、旧 item 明细被新扫描结果完全替换，新增文件可见且已有输出仍为 skipped；失败/取消也只留下同一条最新任务事实。
  - palette menu 四项的色样、文字和尾部选中槽始终在相同列，对勾显隐不引起文字或菜单宽度移动。
  - 返回转换列表是独立可辨识的 sticky pill/button；滚动经过标题、筛选或歌曲行时保持清晰对比。
  - 搜索/歌手/专辑默认紧凑、focus/有值展开，四项清空均位于控件内部；结果/排序不拉伸占满空白。
  - 桌面歌曲行呈现序号/封面/标题歌手/专辑/格式/大小码率/操作列；整行非交互区域播放，hover 将序号替换为 play/pause，歌手/专辑点击可筛选；窄窗口仍无溢出或不可达操作。
  - 分页右对齐且不被左下播放器遮挡。
- Validation:
  - Rust state/history/command tests 覆盖 same-ID reset、row-count stable、old-item replacement、orphan artwork、busy/nonterminal/missing、failure/cancel 和 event run ID。
  - React tests 覆盖 reserved palette slot、compact filter focus/value/clear、row mouse/keyboard/interactive-child click、artist/album filter、back pill class 与 pagination layout hook。
  - macOS 原生 debug/release bundle 在 920×600、1080×760 和长列表中复验截图 1–5 对应场景；Windows UI 留入既有 T6.3 平台矩阵。

### M8 — Viewport-safe 元数据编辑与播放器动效修复

- Goal: 元数据弹窗和播放器在长候选、缺封面、暂停/循环、收展及页面底部状态下始终正确、可见且流畅。
- Dependencies: M7 的列表/pagination grid；沿用 M3/M4 的播放器与 metadata 安全边界。
- Acceptance criteria:
  - metadata dialog 永不超过应用可视高度，中间内容/候选可滚动，header/actions 可见，键盘 focus trap 能到达全部可见操作。
  - 播放器封面始终为圆形；只有 `playing=true` 才旋转，暂停/停止/reduced-motion 时不旋转；换到无封面歌曲立即显示新占位，不闪回或保留旧图。
  - 收起态默认显示唱片封面、hover/focus 显示 play/pause；展开态 play/pause 位于进度条左侧；左右箭头控制收展并有可中断、reduced-motion-aware 动画。
  - 当前歌曲播放结束后从头继续，音量、mute、跨页状态和 metadata release/restore 既有行为不回归。
- Validation:
  - React/media tests 覆盖 `artworkId -> null` URL cleanup、load race、playing/paused classes、native loop、collapsed hover/focus controls、toggle animation/reduced motion 和 progress layout DOM 顺序。
  - Modal tests 覆盖 viewport CSS contract、scroll region、12 个候选、矮窗口键盘访问、Escape/backdrop/Save/Cancel。
  - macOS 原生应用使用有封面→无封面、播放→暂停→恢复→单曲循环、展开→收起、长候选与滚动到底分页场景复验；记录 Windows 缺口但不以 macOS 代替。

### M9 — 主页入口留白与完整 compact filter 文案

- Goal: 详情页 sticky 导航与筛选工具栏在滚动、多语言和窄窗口下保持精致、清楚且不截断自身提示。
- Dependencies: 已完成的 T3.5/T7.3/T7.4；共享 topbar/density token 和三语言 i18n。
- Acceptance criteria:
  - sticky “返回主页”按钮与顶栏在所有滚动位置保留稳定间隔，不触碰顶栏 border，也不覆盖详情标题或歌曲行；中/英/日视觉文案分别为 `返回主页 / Home / ホーム`。
  - 搜索、歌手、专辑在空值且 blur 的 compact 状态完整显示当前语言的 placeholder；focus/有值平滑展开，清空后恢复 compact，工具栏仍能 wrap 且结果/排序不被迫拉长。
  - 键盘 focus、reduced motion、light/dark 和 920×600 窄窗口行为不回归。
- Validation:
  - `HistoryDetail.test.tsx` 与 locale parity tests 覆盖新 key、既有 onBack/query/clear 语义和 compact state hooks。
  - macOS native bundle 逐一切换中/英/日，在 920×600、1080×760 中截图核对 sticky gap 和五个工具栏控件；Windows 结果并入 T6.3。

### M10 — 可见结果序号与可访问的标题/专辑分割

- Goal: 歌曲行使用连续、可理解的界面序号，并让用户在不破坏右侧信息密度的前提下调整标题与专辑空间。
- Dependencies: 已完成的 T7.5/T7.6；现有 server-side filter/sort/page query 保持不变。
- Acceptance criteria:
  - 任意筛选或排序后的完整结果集都从 `01` 连续编号；翻页后从上一页末尾继续，且 hover/active 仍在同一位置替换为 play/pause。
  - 标题/歌手和专辑占满 fixed leading/trailing columns 之外的全部空间；格式、大小+码率和 actions 之间采用一致紧凑 gap，没有 screenshot 3 所示 FLAC 后大段空白。
- 每个对齐的 separator 都可用 pointer 拖动并通过键盘调整同一比例，比例被 clamp、所有可见行同步对齐；820px 以下隐藏专辑列时不残留 handle 或 tab stop。
  - 点击歌手/专辑实际文字才筛选；点击其列内空白播放当前行，编辑/reveal/open 继续隔离。
- Validation:
  - React tests 覆盖 filtered/sorted/page-relative numbering、ratio clamp/keyboard、separator ARIA/窄断点 hook、metadata-link 与 row click 隔离。
  - 原生 QA 使用短/长/未知元数据、筛选后 ordinal 有缺口的真实 25+ 首列表，拖到最小/默认/最大比例并 resize 窗口，核对 ellipsis、列距和命中区域。

### M11 — 播放器覆盖层与收展 icon 几何闭环

- Goal: 播放器在展开/折叠/hover/focus 状态下只显示应有控件，圆形封面和所有 icon 精确对齐。
- Dependencies: 已完成的 T8.2/T8.3；不修改 PlaybackProvider/audio 行为。
- Acceptance criteria:
  - 展开态封面上不存在可见或可命中的 collapsed play/pause 覆盖层；折叠态封面与覆盖按钮同尺寸、同圆心、无多出一圈或偏移。
  - 收展按钮使用 Phosphor direction icon，固定方形 hit target 内水平/垂直居中，方向与 expanded/collapsed 状态一致，并保留 aria-label/title。
  - hover/focus 过渡、封面旋转/暂停角度、native loop、音量和 reduced-motion 均不回归。
- Validation:
  - `PersistentPlayer.test.tsx` 覆盖 expanded hidden contract、collapsed enable/visibility、共享尺寸 class/token、toggle icon/state 和现有 playback assertions。
  - macOS native bundle 在真实封面/占位封面、playing/paused、expanded/collapsed 与 hover/focus 中做高倍率截图；Windows 运行态检查并入 T6.3。

### M12 — 结果内联列标题、可用拖动手柄与折叠播放器精确留白

- Goal: 消除每首歌曲重复的分割线与可访问焦点，把结果计数和列标题合并为一行且让唯一拖动手柄在真实原生 WebView 中可靠工作；同时修正右侧列的视觉间距与折叠播放器唱片/展开按钮的拥挤几何。
- Dependencies: M10 shared ratio/grid and M11 shared player disc token.
- Acceptance criteria:
  - 宽屏列表没有额外独立 header 行：结果计数（及 companion count）占 leading index+cover 区域，同一行依次显示歌曲、唯一 `role=separator`、专辑、格式、大小/码率和明确的操作列；行中没有可见竖线、role separator 或 tab stop，但 title/album/facts/actions 与该行共同 tracks 精确对齐。
  - 唯一 handle 可通过真实 pointer drag、keyboard、ARIA 调整同一 session ratio；测试以有 geometry 的 pointerDown/move 验证 60 变为非 60。WKWebView 若 element pointer capture 不能持续，则 drag 生命周期改由 window listeners 可靠承接；820px 以下 album/handle 同时消失，不保留无效焦点。
  - 有值 artist/album 按钮在真实和自动化环境都能触发独立筛选并重置 page，不触发行播放；unknown 保持非交互。
  - 格式、facts、actions 使用更紧凑但所有行一致的 fixed CSS token tracks；format/facts 视觉上相邻，facts 到 actions 另有明确间隔。长或未知 outcome 不撑开 tracks、会 ellipsis/title 或安全换行，且不溢出。
  - collapsed player 的封面与 expand button 保持同尺寸/圆心，各自 hit target 之间有约 8px 的单一视觉间隔，不因已收起的 0-width expanded grid track 造成双 gap。
- Validation:
  - React tests 覆盖唯一 header separator、行无 separator/tab stop、pointer/keyboard ratio、820px path、header/row shared tracks、长/未知 facts 和 collapsed spacing contract。
  - 完整 Vite+ gates，以及只使用工作区 absolute debug bundle 的 920×600/1080×760 macOS QA；Windows 运行态继续进入 T6.3。

## Task Breakdown

- [x] T1.1 在 `src/formats/audio.rs`、`src/crypto/audio.rs` 和 `src/formats.rs` 增加 checked、seek-based path probe，补齐大 ID3/APE、截断长度、真实 MP3 和伪装文件测试；验证相关 Rust 单测。（实现与测试已写，运行结果由复验记录。）
- [x] T1.2 在 `src/app/job.rs`、`src/app/runner.rs` 和 `src/app/events.rs` 保留 preflight 媒体类别，让标准音频 copy/skipped 生成 `MusicSnapshot`，同时保持 CLI copy/skip 汇总不变；验证 runner/event 测试。（runner 回归已写，运行结果由复验记录。）
- [x] T1.3 扩展 `src/metadata/tags.rs` 的 bounded snapshot/audio properties 以提供 nullable bitrate，并让发布后 facts、历史 repair 和 audio protocol 使用统一 path probe；验证 MP3/FLAC/未知格式测试。（实现已覆盖 `job`、history repair、audio protocol、Rust/TS DTO 和详情显示；path-probe/MP3 与 history 测试已写。）
- [x] T1.4 将 `desktop/src-tauri/src/history/mod.rs` 升级到下一 schema version，加入 `runs.display_name`、`items.bitrate_kbps`、`media_probe_version`、`metadata_edit_pending` 及所需索引，覆盖 v1/v2/新版本幂等迁移。（schema v3 与迁移/reopen 回归已写。）
- [x] T1.5 更新 `desktop/src-tauri/src/commands/mod.rs`、`desktop/src/lib/contracts.ts`、`desktop/src/contracts/bootstrap.fixture.json` 和 DTO tests，传递 `displayName`、`bitrateKbps`、`copied` outcome 与 bounded repair 状态。（repairPending DTO 已接通。）
- [x] T1.6 实现旧 copied/skipped item 的逐批 path-probe repair 和前端空闲刷新，验证千条历史仍限制单批 I/O/内存且最终 counts/facets 准确。（每批32条、repairPending true→false 单 timer 前端回归通过；千条运行压力验证留复验。）

- [x] T2.1 在 `desktop/src-tauri/src/history/mod.rs` 增加名称 fallback、rename 和 run snapshot 读取，校验 terminal 状态及 1–120 字符规则；验证重启持久化和旧库 fallback。（实现与 Rust 回归测试已写。）
- [x] T2.2 重构 `desktop/src-tauri/src/commands/mod.rs`，让新任务与 `rerun_conversion` 共用一个内部执行入口和 state gate；验证新增文件、旧输出 skipped、busy/缺失路径及旧记录不变。（v0.3.0 基线完成；“新建并保留旧记录”已由 D20/T7.2 取代。）
- [x] T2.3 在 `desktop/src/lib/ipc.ts`、`desktop/src/App.tsx` 和历史/详情组件加入 rename/rerun 控件及状态更新，保证操作按钮互不触发详情/删除；验证 React 交互和转换事件 reducer。（按新反馈：homepage 无 rename；详情页标题内联 rename，不依赖 WKWebView 不支持的 `window.prompt`，调用 ID-only API；列表 rerun 保持独立，React 回归通过。）

- [x] T3.1 新建 `desktop/src/features/playback/PlaybackProvider.tsx`（或同等 controller）、类型和 tests，将唯一 audio 生命周期提升到 `App` 页面切换之外；在首次播放前应用默认/持久化音量并实现 clamp、节流保存和静音恢复状态。（首次 play 前 0.6、显式 0、静音恢复、bootstrap 回灌均由前端回归覆盖。）
- [x] T3.2 扩展 Rust/TypeScript `Preferences`、`desktop/src/lib/preferences.ts`、IPC fixture 和 tests，加入默认 0.6 的 `playerVolume`，验证旧/非法值 fallback、显式 0、重启持久化且不会回到 1.0。（Rust fallback/持久化测试已写。）
- [x] T3.3 新建 `desktop/src/features/playback/PersistentPlayer.tsx` 与 `desktop/src/styles/player.css`，实现展开/收起、曲目信息、旋转封面、play/pause、seek、时间、音量 range、静音/恢复、错误和响应式 safe-area；验证键盘音量、reduced motion 与 object URL 清理。（对象 URL/reduced-motion CSS 与前端交互回归已覆盖。）
- [x] T3.4 简化 `desktop/src/features/history/HistoryDetail.tsx` 的 row player 为直接 play/pause 按钮，删除行内 range/time 和“条目离开当前页即 stop”逻辑；验证翻页、筛选、主页往返持续播放且音量不变。（行按钮委托 shared provider 的回归通过。）
- [x] T3.5 新建共享 scroll/navigation hook/component，接入 `desktop/src/App.tsx`、`App.css` 和 `history.css`，实现固定详情主页按钮与阈值返回顶部按钮；验证 smooth/reduced motion、焦点和窄窗口位置。（实现完成；前端 type/test/build 已通过，原生窄窗口 QA 留 T6.3。）
- [x] T3.6 扩展 `desktop/src-tauri/src/audio_protocol.rs` 和 ID-only output lookup 以允许 copied music，覆盖 HEAD/range、失效文件、symlink、删除活动记录与 unknown MIME 测试。（测试已写。）

- [x] T4.1 扩展 `src/metadata/tags.rs` 的 metadata patch/write verification，支持 title/artist/album set/clear 和 front-cover replace，保持 MP3/FLAC 原子候选发布与 audio payload 回归。（MP3 candidate set/clear/cover/坏封面保持原字节测试已写；FLAC 运行回归由复验记录。）
- [x] T4.2 在 `desktop/src-tauri/src/history/mod.rs` 实现 editable-item lookup、pending marker、真实文件 snapshot 回写、artwork dedupe/orphan cleanup、run representative cover 更新与中断 repair；验证两阶段每个失败点。（recovery/artwork history 回归已写。）
- [x] T4.3 在 `desktop/src-tauri/src/history/mod.rs` 实现按同专辑优先的 bounded artwork candidate 查询，覆盖 normalization、query、cursor/limit、SHA 去重和跨 run 结果。（normalized same-album、cross-run SHA dedupe 与 bound 测试已写。）
- [x] T4.4 在 `desktop/src-tauri/src/commands/mod.rs`、`state.rs` 和 `lib.rs` 注册 ID-only metadata/candidate commands，限制字段/bytes、验证 terminal successful music/output、拒绝 active conversion/symlink；更新 security policy tests。（commands/ACL 接线与 Rust validation tests 已写。）
- [x] T4.5 新建 `desktop/src/features/metadata/MetadataEditor.tsx` 及 tests/styles，完成 focus-managed modal、字段编辑、同专辑候选、搜索、lazy artwork、上传预览、保存/error/retry。（前端 modal regression 通过。）
- [x] T4.6 将编辑成功结果同步到当前详情、facets、run representative artwork 和活动播放器；验证编辑当前曲目时 pause/reload 及删除/编辑竞态。（保存后刷新 detail/facets 并 pause active player 的接线完成；Rust/native 竞态门禁留 T6.2/T6.3。）

- [x] T5.1 重构 `HistoryDetail` 四个筛选控件为各自带条件清空按钮，移除 global reset 对排序的副作用；验证 debounce、组合筛选和 page reset。（前端 query/clear regression 通过。）
- [x] T5.2 给 Rust/TypeScript `Preferences` 增加 serde-compatible palette，更新 `desktop/src/lib/preferences.ts`、`theme.ts`、bootstrap fixture 和迁移/缓存 tests。（fixture/TypeScript tests 通过；Rust migration regression 已写。）
- [x] T5.3 扩展 `desktop/src/components/AppHeader.tsx` 和 `desktop/src/styles/tokens.css`，加入 blue/emerald/violet/amber palette menu、light/dark tokens、键盘导航和对比手工检查。（按新反馈改为单 icon dropdown：默认收起、选择持久化、roving keyboard/Escape/外点与焦点恢复均有回归；原生对比手工检查留 T6.3。）
- [x] T5.4 新建 `desktop/src/lib/display-scale.ts`，在 macOS/Windows 读取/监听 Tauri scale factor，使用可单测的平台归一化与 clamp 输出密度 token，验证不把原生 factor 原样用于整页 zoom。
- [x] T5.5 重构 `desktop/src/styles/tokens.css`、`App.css`、`history.css`、player/editor styles：提升字体/图标/控件基线，收紧 drop-field、section gap 和无信息 padding，并验证 920px 最小窗口、1080×760 默认窗口和长文本布局。（CSS density 基线实现及 build 通过；原生窗口矩阵留 T6.3。）
- [x] T5.6 更新 `desktop/src/i18n/locales/{zh-CN,en-US,ja-JP}.json` 与 locale parity tests，覆盖任务、播放器音量/静音、编辑器、独立清空、palette、码率和新错误文案。（locale key parity test 通过。）

- [x] T6.1 更新 `ARCHITECTURE.md` 的 code map、desktop flow、metadata correctness、媒体探测、持久播放器、安全边界和迁移/恢复不变量。
- [x] T6.2 执行完整 Rust、Vite+、Tauri no-bundle validation，逐项把命令、结果、平台和缺口记录到本计划 `Progress`/`Completion Evidence`。（root/desktop Rust gates、frontend gates 与 macOS no-bundle build 已通过；原生 QA 另留 T6.3。）
- [ ] T6.3 在 macOS 和 Windows 完成端到端 QA：macOS absolute debug bundle 已完成新增后重跑、重命名、标准音频/码率、跨页播放及音量、滚动按钮、标签编辑、独立筛选、palette 与密度验证；仅剩 Windows DPI/runtime 和跨显示器矩阵。
- [x] T6.4 独立 review 已于 2026-08-24 通过，全部 10 条 Review Notes 均已 resolved；未请求、亦未创建 commit。计划保持 `In Review`，仅因 T6.3 Windows native QA 尚未完成；除非另有发布请求，不自动创建 tag 或 release。

- [x] T7.1 在 `desktop/src/components/AppHeader.tsx`、`desktop/src/App.css` 和 header tests 中为 palette option 永久渲染固定尾部 check slot，以 `visibility`/`opacity` 表示未选中并验证文字位置不移动。（四项 slot 和 selected 标记回归通过。）
- [x] T7.2 在 `desktop/src-tauri/src/history/mod.rs`、`state.rs`、`commands/mod.rs` 实现 transaction + state-owned same-ID rerun reset，复用既有 conversion pipeline，原子清除旧 items/counters/representative artwork/error 并清理 orphan artwork；覆盖 same ID/count/name/request/timestamps、新旧歌曲、busy/nonterminal/missing/failure/cancel/竞态测试。（direct reset 覆盖独占/共享 artwork、terminal/missing；rerun scan-failure/cancel 与 camel-case event `runId` 回归已覆盖。）
- [x] T7.3 在 `HistoryDetail.tsx`/`history.css` 把返回入口改为可辨识 sticky pill/button，验证滚动重叠、hover/focus、light/dark 和窄窗口。（固定背景/边界/阴影和 focus 合约已接通。）
- [x] T7.4 抽取或实现 compact expandable filter controls，把四个 clear 放入控件尾部并收紧 outcome/sort；覆盖 debounce、focus blur、已有值保持展开、独立 clear、page reset、keyboard 和 reduced-motion。（三个文本筛选采用 focus/value expand，四项 clear 置于 field 内。）
- [x] T7.5 重构歌曲行为媒体资料库 grid：补零序号/hover play、两行 title+artist、独立 album/format/size+bitrate/actions，整行非交互点击播放、artist/album 点击筛选并隔离 actions；覆盖 active/paused/disabled/unknown/long text/responsive tests。（行/子操作隔离与 artist 筛选回归通过。）
- [x] T7.6 将详情 pagination 右对齐并加入与 persistent player/safe-area 协调的响应式间距；补 layout class regression 与原生页面底部 QA。（right-aligned safe-area bottom spacing 已接通；原生 QA 留 T8.5。）

- [x] T8.1 把 `MetadataEditor.tsx`/`history.css` 拆成 viewport-bounded dialog shell、可滚动 body/candidate region 和固定 actions，保持 focus trap/object URL cleanup，并覆盖长候选与矮 viewport tests。（header/body/actions contract 与 internal scroll 已覆盖。）
- [x] T8.2 修复 `PersistentPlayer.tsx` artwork 生命周期：ID 变化即清空旧 URL、防止迟到请求回写、null/error 显示当前占位；封面使用圆形且旋转 class 只绑定 `playing`，覆盖 cleanup/race/pause/reduced-motion tests。（player/detail 的 URL replacement/race/null 回归已覆盖；真实封面暂停时维持既有 animation identity、只改 play state。）
- [x] T8.3 重排播放器 expanded/collapsed DOM/CSS：播放按钮位于进度条左、collapsed 唱片 hover/focus overlay、左右箭头与流畅/reduced-motion 收展；给唯一 `<audio>` 启用单曲 `loop` 并覆盖 ended/replay、音量和跨 route 回归。（expanded content 保持 mounted、以 inert 防止重复焦点，容器/content transition 已接通；cover running→paused style 回归已覆盖。）
- [x] T8.4 更新中/英/日可访问文案与 parity tests（筛选展开、序号播放、播放器左右收展等），并复验所有 icon-only 控件 title/aria-label。（复用已有三语言可访问 key；新增 icon-only controls 均有 title/aria-label，locale parity 纳入完整 VP test。）
- [x] T8.5 已更新 `ARCHITECTURE.md` 的 rerun 生命周期为 same-ID transactional reset；完整 Rust/Vite+/Tauri gates 与工作区 absolute debug bundle 的 macOS 原生截图场景 QA 均通过。Windows T6.3 外部缺口保持不变，不以 macOS 证据替代。

- [x] T9.1 在 `desktop/src/styles/tokens.css`、`desktop/src/App.css` 与 `desktop/src/styles/history.css` 统一 topbar height/sticky gap token，更新 `HistoryDetail.test.tsx` 的 sticky hook，验证滚动后按钮与顶栏始终有间隔且 920×600 不遮挡正文。（`--topbar-block-size` 和 `--detail-sticky-gap` 已取代两个脱节的 `68px`；native 920×600 QA 合并到 T11.3。）
- [x] T9.2 在 `desktop/src/i18n/locales/{zh-CN,en-US,ja-JP}.json` 将详情主页入口改为 `返回主页 / Home / ホーム` 的专用 key，更新 locale parity、`App.test.tsx`/`HistoryDetail.test.tsx` 查询并验证 onBack/scroll reset 不变。（`app.backHome` 已接入详情按钮；locale parity 和详情交互回归通过。）
- [x] T9.3 重构 `history.css` 的 compact control size variables，使搜索/歌手/专辑的 collapsed minimum 能完整容纳三语言 placeholder、图标和 clear slot，同时保留 focus/value expand、wrap 和 reduced-motion；运行 focused frontend tests 并完成三语言原生截图 QA。（collapsed search 178px、artist/album 184px，expanded 统一 clamp；focused frontend tests 通过，三语言 native screenshot QA 合并到 T11.3。）

- [x] T10.1 在 `desktop/src/features/history/HistoryDetail.tsx` 用 `(page - 1) * pageSize + map index + 1` 而不是 `item.ordinal` 渲染序号，必要时抽取纯 helper；在 `HistoryDetail.test.tsx` 覆盖筛选/非 ordinal 排序第一页从 `01` 开始、第二页从 `pageSize + 1` 开始，以及 hover/active 替换图标。（`visibleItemNumber()` 位于纯 `layout.ts`；含 ordinal 缺口、非 ordinal 排序和 page 2 的 `26` 回归。）
- [x] T10.2 在 `HistoryDetail.tsx` 和 `history.css` 实现共享 `titleAlbumRatio`、每行相同语义的 pointer-capture/keyboard `role="separator"` 与 clamp 逻辑；所有行通过同一 CSS variable 对齐，820px 以下移除 separator。测试 pure clamp、Arrow/Home/End、ARIA、pointer up/cancel release 和窄断点状态。（35–75 clamp、pointer capture/release/cancel、Arrow/Home/End、ARIA 和每行共享 CSS tracks 已接通；820px hook 不渲染 separator/tab stop。）
- [x] T10.3 调整 `history.css` 的 media row grid：标题/专辑吃满唯一 flexible region，format/facts/actions 使用稳定 fixed tracks 和统一 gap；歌手/专辑按钮使用 intrinsic hit area。补 row click isolation 回归，并用长/短元数据在最小/默认/最大 ratio 下完成原生布局验证。（右侧三轨统一为稳定 CSS token，不随每条 facts/outcome 文本改变；metadata buttons 使用 `fit-content`/`justify-self:start`/`max-inline-size`，长文本有 ellipsis/title、未知值为非交互文本；row/action isolation、long/short tracks 和 keyboard/sequencing tests 通过。默认 ratio 的实际短/unknown/有值 metadata layout 已在 T11.3 的 30 首 fixture 完成；真实超长 metadata 与 pointer 极值的 native 截图仍是非阻断的手工跟进，不能被上述 default-state QA 冒充。）

- [x] T11.1 在 `desktop/src/features/playback/PersistentPlayer.tsx` 与 `desktop/src/styles/player.css` 统一 disc/overlay 尺寸，expanded 态以 visibility/pointer/focus 强契约隐藏 collapsed overlay，collapsed hover/focus 才显示；在 `PersistentPlayer.test.tsx` 覆盖真实/占位封面和 expanded/collapsed 状态。（disc 与 overlay 共用 `--player-disc-size`；expanded 时 `disabled + tabIndex=-1 + aria-hidden + visibility:hidden + pointer-events:none`，collapsed 时才可达。）
- [x] T11.2 将收展入口替换为 Phosphor `ArrowLineLeft/ArrowLineRight`（或 native QA 通过的同类方向 icon）和专用 centered icon-button class，保留 label/title/reduced-motion；验证方向、尺寸、focus ring 和不再出现形似原始 `<>` 的偏心视觉。（采用 `ArrowLineLeft/Right` 与固定 42px `.player-collapse-toggle`，通过 `data-collapse-direction`/aria/title 回归状态；native 高倍率视觉检查留 T11.3。）
- [x] T11.3 执行本轮完整 Vite+ format/check/test/build、Tauri macOS debug app bundle 与 `git diff --check`，使用用户四张截图对应场景完成 920×600/1080×760 native QA；把命令、截图结论和仍开放的 Windows T6.3 缺口写回 `Progress`/`Completion Evidence`。（完整 VP gates、absolute debug bundle 和三语言/两尺寸 native QA 均完成；debug bundle 的临时 sealed-resources 签名提示不作为 release 签名证据，Windows T6.3 保持开放。）本轮纯前端边界不要求修改 `ARCHITECTURE.md`，除非实现改变已记录的播放器所有权或 history query 契约。

- [x] T12.1 在 `HistoryDetail.tsx`、`history.css` 和中/英/日 locales 中用与 rows 相同 CSS tracks 渲染紧凑列 header；将唯一 pointer/keyboard/ARIA separator 移入 header，行中改为非视觉、aria-hidden spacer。覆盖唯一 handle、行无 separator/tab stop、header/row ratio alignment、pointer capture/keyboard 和 820px hidden path。（header/row 均为 `.song-layout-grid` 和同一 `--music-grid-template`；定向 HistoryDetail 22 tests 覆盖唯一 separator、ratio、pointer lifecycle 与窄断点。）
- [x] T12.2 收紧 `history.css` 的 format/facts/actions stable tokens，处理长/未知 outcome 文本；在 `player.css` 为 `.persistent-player.is-collapsed` 的 disc/toggle 建立单一约 8px 间隔，避免 0-width expanded track 与 grid gap 双算。补 compact token/ellipsis 与 collapsed spacing tests。（tracks 为 64px/78px/112px，1080px 以下 60px/76px/112px；outcome 有 title/ellipsis contract，action target 为 36px；collapsed toggle 的 8px margin 不依赖 grid gap。）
- [x] T12.3 执行 focused 和完整 Vite+ format/check/test/build、`git diff --check`、Tauri macOS debug bundle，并在绝对工作区 bundle 的 920×600/1080×760 复验唯一 header handle、对齐列、compact trailing tracks 与 collapsed player 间隔；更新本计划并进入独立 review-plan。不得使用 `/Applications/UnlockM.app`，Windows T6.3 保持开放。（focused 3 files/27 tests，full 17 files/70 tests，absolute debug bundle QA 完成；未使用 `/Applications`。）
- [x] T12.4 根据真实运行反馈重构 `HistoryDetail.tsx`/`history.css`：将结果/companion count 与列标题合并为同一共同 grid 行，增加三语言操作列标题；让唯一 separator 的真实 pointer drag 在 WKWebView 可靠更新 ratio，修复有值 artist/album 的筛选命中/事件隔离，并重新平衡 format/facts/actions 对齐。补 geometry pointer、artist/album query/non-playback、唯一 header、820px、long/unknown 和 spacing tests；用绝对 workspace bundle 在 920×600/1080×760 实拖并点击真实 metadata，随后独立复审。不得使用 `/Applications`，保持 collapsed 8px 与 Windows T6.3 缺口。（window pointermove/up/cancel fallback、24px transparent hit target/2px visible line、inline count/labels/actions、artist/album explicit isolation 与 visual trailing alignment 均完成；full VP 71 tests 和 native 60→49 drag/QA Artist+Album filter 通过。）
- [x] T12.5 修复 CSS custom-property 解析边界：ratio variables 与引用它们的 `--music-grid-template` 必须在同一 `.song-list` element 计算，header/rows 不再各自写 inline ratio style；补 parent-owned ratio/template regression，并以真实 native 前/后 separator x 坐标证明拖动改变实际列宽。恢复完整中/英/日 facts 标题，删除 facts→actions 单独 margin，以同一 uniform grid gap 让 format—facts 和 facts—actions 视觉间隔一致，同时调整固定 facts track 与对齐。保持 count/header/actions、artist/album 和 collapsed 8px；完整验证/absolute bundle QA 后重新独立 review。（ratio inline style 现仅在 `.song-list`，focused 26/full 71 tests 通过；absolute bundle 1080×760 实拖 separator x≈485→385、60→41，920×600 验证完整 facts/actions 和等距 trailing layout。）

## Validation Strategy

### Automated core and CLI

- `rtk cargo fmt --all -- --check`
- `rtk cargo clippy --locked --all-targets --all-features -- -D warnings`
- `rtk cargo test --locked --all-targets --all-features`
- `rtk cargo test --locked --test watch_recursive -- --test-threads=1`
- `rtk cargo test --locked --test watch_stability -- --test-threads=1`

重点断言：标准音频与 companion 分类、copy disabled 的 music skipped row、copied music snapshot、path probe 对大/恶意 ID3、nullable bitrate、metadata set/clear/cover replace、原子失败保持原输出、source SHA 不变。

### Automated desktop frontend

- `rtk vp -C desktop fmt . --check`
- `rtk vp -C desktop check --no-fmt`
- `rtk vp -C desktop test`
- `rtk vp -C desktop build`
- `rtk git diff --check`

重点断言：IPC fixture parity、same-ID rerun 状态、全局单例播放器跨 route/page/filter、60% 默认/持久音量和静音恢复、native loop、无封面切换、playing-only 封面旋转、expanded overlay 强隐藏/折叠封面同圆心、媒体资料库 row、筛选/排序结果跨页连续序号、title/album ratio clamp 与可访问 separator、文本级 metadata hit area、fixed navigation/sticky gap、三语言完整 placeholder、compact filters/内部独立清空、viewport-safe metadata modal、palette reserved check slot、macOS/Windows density token 和三语言 key parity。

M9–M11 是纯 React/CSS/i18n 修复，以上五项与下方 macOS app bundle 是最低门禁；若实现触及 Rust、Cargo、IPC contract 或 `ARCHITECTURE.md` 边界，则重新执行对应 core/desktop Rust 全量门禁，不用旧证据替代。

### Automated desktop Rust and packaging

- `rtk cargo clippy --locked -p unlockm-desktop --all-targets -- -D warnings`
- `rtk cargo test --locked -p unlockm-desktop --all-targets`
- `rtk cargo test --locked -p unlockm-desktop --test security_policy`
- `rtk cargo test --locked -p unlockm-desktop --test license_audit`
- `rtk vp -C desktop exec tauri build --debug --no-bundle`
- `rtk vp -C desktop exec tauri build --debug --bundles app`（macOS native QA 使用该工作区绝对 bundle，不启动 `/Applications` 中可能存在的旧版本）

重点断言：v1/v2/new schema migration、bounded legacy repair、run rename/snapshot、same-ID rerun reset/state gate/old-item replacement/orphan cleanup、ID-only copied playback、metadata pending recovery、artwork candidate query、symlink/invalid bytes rejection，以及 capability/CSP 未扩大。

### Manual native QA

1. 转换包含 encrypted music、普通 MP3/FLAC、大 ID3 MP3、非音频 companion 和坏 `.mp3` 的目录；核对列表数量、copied/converted/skipped、格式和码率。
2. 记录任务总数和 run ID，在源目录新增音乐后重跑；确认使用原输出/设置、只产生新输出、返回相同 ID、总数不变、名称保留、旧歌曲明细被新扫描事实替换且该 ID 的列表位置保持稳定。重命名后重启应用确认持久化。
3. 首次播放前确认音量为 60% 而不是 100%；调整音量、静音/恢复后翻页、改变筛选、回主页、再进详情并 seek，音频和音量均不中断/重置。顺序验证有封面→无封面、播放→暂停→恢复→自然播完循环；展开态封面上没有 collapsed overlay，折叠态封面/覆盖按钮同圆心，`ArrowLineLeft/Right` icon 居中且 hover/focus/动画正确。
4. 长列表滚动后验证右下返回顶部，详情验证固定左上“返回主页” pill/button 与顶栏保留 8–12px 视觉间隔；键盘、reduced motion、窄窗口下均可达且不与内容重叠。滚到底确认右对齐分页不被播放器遮挡。
5. 分别编辑 MP3/FLAC 的三字段和封面，测试同专辑建议、搜索已有封面、合法上传、坏图/超限图；在 920×600 矮窗口确认候选/表单可滚动且 Save/Cancel 始终可见，再用外部标签读取器复核并确认源文件 hash 不变。
6. 中/英/日逐个验证搜索、歌手、专辑在未 focus 时完整显示 placeholder，focus/值时展开；组合筛选并只点击输入框内部的一个 `×`，确认其余值和排序保持。筛出原 ordinal 不连续的歌曲并切换 title/artist/album/size 排序，确认完整结果从 `01` 连续编号；当前 `pageSize=25` 时第二页从 `26` 开始。
7. 检查 blue/emerald/violet/amber × light/dark、三种语言、长路径/长标题、无封面和错误态；逐项切换 palette 时菜单文字列完全不移动。
8. macOS 默认/更多空间/较大文字及可用的内外接显示器、Windows 100%、125%、150%、200% 及跨 DPI 显示器移动，检查字体/图标/控件均更易读、主要页面空白减少、工具栏/modal/player/fixed controls 无裁切遮挡，并确认没有 native factor 二次放大。
9. 在 1080×760 及 920×600 下拖动标题/专辑 separator 到最小、默认、最大值，再 resize 窗口；确认全部行同步、format/facts/actions 等距稳定。只点歌手/专辑文字应筛选，点同列文字外空白应播放行，三项文件操作仍不触发播放。

## Risks and Mitigations

- **MP3 标签可声明巨大或越界长度。** Path probe 仅 checked-seek，不按声明长度分配；限制标签层数、校验 file length，并保留连续帧验证，防止伪装扩展误判。
- **把标准音频从 companion 改为 music 会改变旧详情数量。** 用 `media_probe_version` 有界懒修复和 repair-pending 状态逐步校正；不依据扩展名直接批量改库。
- **文件发布与 SQLite 无法组成单一事务。** 先写/验证 private candidate，再持久化 pending marker，再原子 publish，最后从实际输出回读并清 marker；启动/详情读取负责恢复中断点。
- **Windows 正在播放的文件可能被 WebView2 占用。** 编辑活动 item 前暂停并释放 source，Rust 仍返回稳定 busy/output error；候选不直接覆盖打开句柄，实际 Windows QA 验证 replace 行为。
- **全局播放器可能持有已删除的 item ID。** UI 删除前停止匹配 track，protocol 每个请求仍重新验证 history/output；删除或文件变化会终止并显示可恢复错误。
- **Retina/WebView2 已应用设备缩放，原生 factor 直接相乘会二次放大。** macOS/Windows 共用更大的视觉基线；不使用 CSS `zoom`/transform，只用平台归一化、可 clamp/单测的 factor 推导密度 token，并在两端多缩放率/跨显示器实际校准。
- **音量初始化晚于第一次播放会短暂以 100% 输出。** provider 在设置 `src`/调用 `play()` 前同步写入经过 clamp 的默认或持久化值；缺失/坏 preference 固定回落到 0.6，并用单元测试断言调用顺序。
- **新增控件会挤压 920px 最小窗口。** 播放器、toolbar、run actions 和 modal 使用响应式断点/safe-area；固定控件位置纳入视觉 QA。
- **本地封面搜索可能泄漏大 blob 或拖慢查询。** 列表只返回 metadata/ID，SQL limit/cursor + SHA 去重，图片沿用 lazy byte command 和 10 MiB 上限。
- **preferences/IPC 字段扩展可能破坏旧库或前端 fixture。** Rust 新字段使用 serde default，migration/reopen 和 Rust-generated fixture parity 是 release gate。
- **原位重跑先清旧条目会失去旧执行快照。** 这是用户明确要求的替换语义；reset 在单个 SQLite transaction 内完成且仅在输入/输出重新验证、目标仍 terminal、无其他 active/mutation 后执行。后续转换失败时同一 run 明确保留新的 failed/cancelled 事实，不伪装回旧结果。
- **same-ID 重跑与 delete/clear/metadata edit 可能竞态。** `begin_rerun` 必须使用与 conversion start/history mutation 相同的 active mutex，并在锁内完成 terminal recheck + transaction reset；并发测试以 barrier 验证不能交错。
- **整行播放容易吞掉歌手/专辑或文件操作。** 仅非交互区域的鼠标 click 冒泡到 row handler；原生 button/link/input/select 以及 action 容器都排除，序号按钮保留完整键盘语义和可见 focus。
- **坍缩筛选可能隐藏当前状态或造成横向抖动。** 有值时强制展开并显示独立 clear；动画只改变受 clamp 的 inline-size，不改变 label 顺序，reduced-motion 下即时切换，窄窗口允许 toolbar wrap。
- **播放器动画可能持续耗电或错误表达状态。** 旋转严格由 `playing && artwork` 控制，暂停/结束/reduced-motion 禁用；收展动画只作用于小型固定容器并可中断，不对整页使用 transform/zoom。
- **本地化 placeholder 与 compact 密度相冲突。** 以中/英/日最长受支持文案决定每类 collapsed minimum，允许 toolbar wrap，不通过 overflow/ellipsis 隐藏控件用途；原生 WebView 截图是验收门禁。
- **界面序号可能被误解为持久化扫描编号。** 明确它只表示当前筛选/排序结果中的分页位置：结果集从 1 开始并跨页连续；后端仍用 `item.ordinal` 提供“原始顺序”排序，展示序号不持久化，避免 schema/query 变更。
- **拖动分栏可能挤掉右侧事实或误触播放。** 只分配 fixed columns 之外的 flexible region，ratio 双侧 clamp；separator 使用 pointer capture、独立 hit target 和 keyboard semantics，并拦截 row click。820px 以下隐藏专辑列时完全移除 handle。
- **grid item 默认 stretch 会扩大歌手/专辑误触区。** 文本按钮显式 intrinsic sizing/左对齐/max-width，ellipsis 只裁视觉文本；空白区域交回 row playback，并用 pointer QA 验证真实 hit box。
- **仅把播放器覆盖层设为透明仍可能残留合成层或焦点。** expanded 态同时使用 visibility、pointer-events、disabled/tab-order 契约；disc/overlay 共享单一尺寸 token 和圆形 box model，高倍率原生截图检查边缘与圆心。

## Progress

- [x] 2026-08-25：主代理在 T12.5 re-review Pass 后完成最终独立复验：`rtk vp -C desktop fmt . --check`（58 files）、`rtk vp -C desktop check --no-fmt`（40 files clean）、`rtk vp -C desktop test`（17 files / 71 tests）、`rtk vp -C desktop build` 与 `rtk git diff --check` 全部通过；build 仅保留既有 ineffective dynamic import 和超过 500 kB chunk 的非失败建议。Computer Use 只操作绝对工作区 bundle `/Users/peilin/Codebase/um-rewrite/unlockm/target/debug/bundle/macos/UnlockM.app`：在 920px 窗口把 separator 从约 x=320/AX 41 拖至约 x=448/AX 75，标题/专辑列同步重排；同一截图确认“30 首歌曲”与列标题同一行、完整“大小与码率”/“操作”、format/facts/actions 对称 12px grid gap，以及有值 `QA Artist`/`QA Album` 的可访问按钮。未启动 `/Applications/UnlockM.app`。
- [x] 2026-08-25：完成 T12.5，状态回到 `In Review` 等待原 M12 reviewer 再审。`mediaRowStyle` 现在只写入 `.song-list`，header/rows 无局部 ratio override；同一 parent 同时拥有 ratio 与 `--music-grid-template`。tests 断言 parent 的 40/60→70/30 更新及 children 无错误 style。facts labels 已恢复为 `大小与码率 / Size & bitrate / サイズとビットレート`，actions margin 删除，所有尾列以 12px grid gap 排列，format 向 facts、facts 居中、actions 靠左。focused HistoryDetail/locale 26 tests、full VP fmt/check/test/build（17 files / 71 tests）与 diff-check 通过。为避免旧运行进程混淆，确认并关闭旧 workspace debug PID 后只重新启动 `/Users/peilin/Codebase/um-rewrite/unlockm/target/debug/bundle/macos/UnlockM.app`；1080×760 实测 line x≈485 到 x≈385、ARIA 60→41，920×600 显示完整 facts/actions 与等距尾列。Windows T6.3 和真实超长 metadata/pointer 极值仍开放。
- [x] 2026-08-25：主代理源码分析与用户实测发现 T12.4 的“AX 60→49”不是视觉布局通过证据：`--music-grid-template` 定义在 `.song-list` 父级并在那里解析 `--title-track/--album-track`，而 ratio inline style 写在 header/row 子级，父级已经使用 fallback，子级不能反向改变已计算 template。新增 D36/T12.5，状态重开为 `In Progress`；旧 T12.4 runtime review 只保留为事件/筛选修复历史，视觉 drag/间距结论被 superseded。
- [x] 2026-08-25：完成 T12.4 runtime remediation，计划回到 `In Review` 等待原范围独立复审。真实 root cause 是此前单元素 capture 回归没有几何也未断言 ratio，且 WKWebView 不能据此保证持续 move；现为 pointerDown focus + capture，并以 window `pointermove`/`pointerup`/`pointercancel` 接管拖动生命周期。focused HistoryDetail/locale tests（26 tests）和 full `rtk vp -C desktop fmt . --check`、check（40 files clean）、test（17 files / 71 tests）、build、`git diff --check` 都通过。Tauri debug bundle 仅在绝对工作区路径 `/Users/peilin/Codebase/um-rewrite/unlockm/target/debug/bundle/macos/UnlockM.app` 重建/启动；1080×760 和 920×600 实测将 handle 从 `60` 拖到 `49`，点击 QA Artist 后 artist filter value 为 QA Artist/结果 1 首，再点击 QA Album 后 album value 为 QA Album/结果仍 1 首，未混用 `/Applications`。header count+labels 单行、Actions 标题、format↔facts 紧邻、facts→actions 12px gap 与 collapsed player 8px 都在截图复验；Windows T6.3 与真实超长 metadata/pointer 极值仍开放。
- [x] 2026-08-25：用户真实运行截图推翻了 M12 的静态/初次原生通过结论：额外 header 行造成冗余，header separator 在 WKWebView 中不能可靠拖动，真实有值 artist/album 未触发筛选，且 trailing columns 的对齐仍不符合视觉要求。新增 T12.4/D35，状态重开为 `In Progress`；先更新计划，尚未实现 runtime remediation。
- [x] 2026-08-25：完成 M12/T12.1–T12.3，状态回到 `In Review` 等待独立 review-plan。`HistoryDetail` 以一个 `.song-list-header` 唯一承载 `role=separator`、ARIA、pointer capture/keyboard；全部歌曲行只保留 `aria-hidden` spacer，并与 header 共用 `--music-grid-template` 和 session ratio。format/facts/actions fixed tokens 收紧为 64px/78px/112px（中等宽度 60px/76px/112px），outcome long/unknown 有 ellipsis/title 边界；collapsed player toggle 使用独立 8px start margin，避免已收起的 0-width expanded track double gap。定向 `rtk vp -C desktop test src/features/history/HistoryDetail.test.tsx src/features/playback/PersistentPlayer.test.tsx src/i18n/locales.test.ts` 通过（3 files / 27 tests）；最终 `rtk vp -C desktop fmt . --check`、check（40 files clean）、test（17 files / 70 tests）、build 和 `rtk git diff --check` 通过。使用且只使用 `/Users/peilin/Codebase/um-rewrite/unlockm/target/debug/bundle/macos/UnlockM.app` 重建/启动原生 QA：1080×760 与 920×600 下确认 header 唯一竖线/handle、行内无竖线、header/rows 对齐、三语言标题、compact trailing tracks 及 collapsed 8px 留白；未使用 `/Applications`。Vite warnings 仍为既有 dynamic-import/chunk-size 建议，Windows T6.3 与真实长 metadata/pointer 极值 native 跟进保持开放。
- [x] 2026-08-25：根据新增截图反馈，在完成 M9–M11 plan accuracy cleanup 后增加 M12/T12.1–T12.3 与 D34。用户明确覆盖此前 D32 的每行 handle 选择：唯一可访问 separator 将移到对齐的 column header，歌曲行只保留无视觉/无 tab-stop spacer；同时收紧 trailing fixed tokens 并为 collapsed player disc/expand button 加不双算的约 8px 留白。尚未修改生产代码或勾选 T12 任务；本轮状态重开为 `In Progress`。
- [x] 2026-08-25：完成 T11.3 集成门禁并把计划转为 `In Review`，等待 M10/M11 独立 review-plan。`rtk vp -C desktop fmt . --check`、`rtk vp -C desktop check --no-fmt`（40 files clean）、`rtk vp -C desktop test`（17 files / 69 tests）、`rtk vp -C desktop build` 与 `rtk git diff --check` 通过；Vite 保留既有 ineffective dynamic import 与超过 500 kB chunk 的非失败建议。`rtk vp -C desktop exec tauri build --debug --bundles app` 首次在沙箱内因已配置 sccache 的 `Operation not permitted` 失败，保持 wrapper 后获准提权重跑，生成并启动绝对工作区 bundle `/Users/peilin/Codebase/um-rewrite/unlockm/target/debug/bundle/macos/UnlockM.app`；debug bundle 的 sealed-resources 校验提示只记录为 debug 签名限制，不伪作 release 签名证据。原生 QA：1080×760 与 920×600 下验证 sticky `Home/返回主页/ホーム` 与顶栏留白、英/中/日完整 search/artist/album placeholder、30 首列表的 `01` 起点/第二页连续 `26`–`30`、稳定 format/facts/actions 列距、unknown metadata 非交互、expanded player 无封面覆盖层、collapsed disc/overlay 同圆心及居中 ArrowLine 收展 icon；播放音量保持 0.6，分页和返回顶部按钮均可达。实际长 metadata 与 separator pointer 极值截图未在此 fixture 补做，保留为非阻断手工跟进；Windows DPI/runtime/跨显示器 T6.3 仍是平台缺口。
- [x] 2026-08-25：M10 独立审查提出的实现 finding 已修复并等待原范围复审：右侧 format/facts/actions 采用所有行共享的稳定 token tracks，long/short facts regression 确认 title/album ratio 同步；artist/album 文字按钮显式 `justify-self:start`、ellipsis/title，未知 metadata 改为非交互文本。separator 的 pointer capture 现在在 up/cancel 时 release，并处理 lost-capture；820px `matchMedia` hook 回归断言不渲染 separator/tab stop。`rtk vp -C desktop fmt src/features/history/HistoryDetail.tsx src/features/history/HistoryDetail.test.tsx`、`rtk vp -C desktop test src/features/history/HistoryDetail.test.tsx`（1 file / 21 tests）、`rtk vp -C desktop check --no-fmt`（40 files clean）和 `rtk git diff --check` 均通过。M10 原生长/短元数据、ratio 极值及 resize QA 仍明确合并到 T11.3，不能视为已完成的 native evidence。
- [x] 2026-08-25：完成 T11.1–T11.2 的代码与 focused regression。播放器 disc/overlay 共用 42px token；expanded 态 overlay 为 disabled、`tabIndex=-1`、`aria-hidden`、`visibility:hidden`、`pointer-events:none`，collapsed hover/focus 才显示。收展入口已由 caret 替换为 `ArrowLineLeft/Right`，专用 icon-button 几何固定 42px 居中。`rtk vp -C desktop test src/features/playback/PersistentPlayer.test.tsx` 通过（1 file / 2 tests），`rtk vp -C desktop check --no-fmt` 与 `rtk git diff --check` 通过；T11.3 的全量 gates/native QA 尚待执行。
- [x] 2026-08-25：M10 的代码任务（T10.1–T10.3）已完成。展示序号由 `visibleItemNumber(page, pageSize, index)` 派生，任何筛选或排序后的第一页均为 `01`、默认 25 条分页的第二页为 `26`；`item.ordinal` 仍只留给服务端原始排序。标题/专辑用同一会话级 60:40 CSS `fr` tracks，并新增 35–75 clamp 的 pointer-capture、Arrow/Home/End `role=separator`；右侧 format/facts/actions 为所有行共享的稳定 fixed CSS token tracks，metadata 文本按钮改为 intrinsic hit area。focused tests/check/diff 结果与 M9 相同（2 files / 21 tests）；真实长/短元数据、ratio 极值与 resize native QA 留 T11.3。
- [x] 2026-08-25：M9 的代码任务（T9.1–T9.3）已完成：topbar/sticky offset 改为共享 `--topbar-block-size + --detail-sticky-gap`，详情入口使用 `app.backHome`（返回主页 / Home / ホーム），compact 搜索/歌手/专辑的折叠宽度按最长三语言 placeholder 扩至 178/184px，focus/value 仍使用同一可动画 clamp。2026-08-25 首次审查发现空值时仍预留 clear 尾 padding 会压缩日语输入区，已改为仅 `data-has-clear=true` 才保留 34px，并增加三语言 placeholder/clear-slot contract。`rtk vp -C desktop test src/features/history/HistoryDetail.test.tsx src/i18n/locales.test.ts` 通过（2 files / 21 tests），`rtk vp -C desktop check --no-fmt` 与 `rtk git diff --check` 通过；M9 的三语言 native/920×600 截图尚未完成，仍是 T11.3 集成门禁，M9 不能据此单独宣告验收完成。
- [x] 2026-08-25：用户更正 D31：展示序号不是每页重置，而是每个筛选/排序结果从 `01` 开始并跨页连续，按 `(page - 1) * pageSize + visibleIndex + 1` 派生；例如每页 10 条时第二页从 `11` 开始。该序号不持久化，后端 ordinal/query 不变。
- [x] 2026-08-25：收到 v0.3.1 的四张 UI 截图反馈；按 `plan-doc` 和 `frontend-design` 规则读取仓库指导、`docs/PLANS.md`、完整 active ExecPlan、当前 `HistoryDetail`/`PersistentPlayer`/CSS/i18n/tests，并在同一计划新增 M9–M11/T9.1–T11.3。计划从 `In Review` 重开为 `Ready`；本阶段只编辑计划文档，未修改生产代码、未运行构建、未创建 commit/tag/push。
- [x] 2026-08-24：用户授权将本轮 reviewed 修复发布为 `0.3.1`：创建本地 annotated `v0.3.1` tag、仅推送 `main` 提交到 `origin`/`github`（明确不推送新 tag），并仅在本机编译、ad-hoc 签名和安装 macOS app。版本权威文件已统一准备为 `0.3.1`；Windows T6.3 不因此关闭。
- [x] 2026-08-24：独立 review-plan re-review 返回 Pass with Notes、无剩余代码 findings；两条新增 Minor Review Notes 均由 reviewer 验证并勾选 resolved。独立命令：rerun 定向 Rust 7 tests、PersistentPlayer/PlaybackProvider 7 tests、`git diff --check` 全部通过。计划继续 `In Review`，只因既有 Windows T6.3 外部 QA 缺口。
- [x] 2026-08-24：review remediation：新增 same-ID rerun scan-failure/cancellation event-stream tests。scan-failure 在 reset 后同 ID terminal Failed、旧 facts 清空、历史计数稳定、active slot release 且无误导事件；cancel hook 在 mark-running 后触发，捕获到的所有 camel-case `runId` 为原 ID，Completed `cancelled=true`。修正 `ConversionEventDto` fields 的 serde camel case。播放器真实 artwork 保持 spin animation，暂停只切换 `animation-play-state: paused`，不重置角度。`rtk cargo test --locked -p unlockm-desktop --lib rerun_ -- --test-threads=1`（7 tests）与 player test（2）通过。
- [x] 2026-08-24：主代理完成最终自动化复验：`rtk cargo fmt --all -- --check`、workspace clippy `-D warnings`、`rtk cargo test --locked --all-targets --all-features`（150 passed / 14 suites）、VP fmt/check/test（17 files / 63 tests）/build、`rtk vp -C desktop exec tauri build --debug --bundles app` 与 `rtk git diff --check` 全部通过。Vite dynamic-import/chunk-size 仍为非失败建议。
- [x] 2026-08-24：主代理用 absolute workspace bundle `/Users/peilin/Codebase/um-rewrite/unlockm/target/debug/bundle/macos/UnlockM.app` 完成 macOS 原生 QA：same-ID 重跑后任务数保持 1 且 30 首明细刷新；palette 固定 check 列；sticky Back pill；compact filter focus/值展开及内部 `×`；媒体资料库行/hover/歌手专辑筛选；圆形真实封面、暂停/循环、真实封面→无封面清理、左右收展；920×600 metadata dialog 内部纵向滚动且 actions 固定；右对齐分页不被展开播放器遮挡。QA 先发现 collapsed inert grid 留下空白竖卡片及 modal 泛化 flex 造成横向表单，两项均修复、补测试、重建并复验通过。
- [x] 2026-08-24：native macOS QA 发现 `.metadata-modal div` 的过宽 descendant flex 规则覆盖 body grid，导致 1080×760 下表单横向排列并出现水平滚动。移除该规则，限定 flex 到 `.metadata-actions`，并让 body 显式 vertical-grid/hidden-x contract。MetadataEditor 回归验证 body/actions sibling 与运行时 display/overflow，`rtk vp -C desktop fmt . --check`、check、metadata test（5）、full test（63）/build 与 `git diff --check` 均通过；Vite warning 仍为非失败建议。
- [x] 2026-08-24：native macOS QA 发现 collapsed player 的 inert expanded grid 仍参与 block sizing，形成空白竖卡片。修复为 mounted expanded content 的显式 `block-size`/`max-block-size` 收展到 0，并改用零像素 grid track；保留可中断 transition 与 reduced-motion instant fallback。PersistentPlayer contract 回归覆盖 dedicated collapsed-content class，`rtk vp -C desktop fmt .`、check、player test（2）、full test（62）和 build 均通过，`rtk git diff --check` 通过。
- [x] 2026-08-24：review remediation：rerun 前通过 shared `PlaybackController.releaseForRun` 释放同 run 音源，失败不恢复 stale item；补 reset 的 direct SQLite regression（facts/request/shared-orphan artwork/nonterminal/missing），以及 PersistentPlayer UI 回归（null/race artwork、playing class、layout/arrow/progress/focus contract）。`rtk cargo fmt --all -- --check` + direct reset test、`rtk vp -C desktop fmt .`/check + focused 4-file test（25 tests）均通过。
- [x] 2026-08-24：T8.5 的 architecture 文档与自动 gates 已完成；当前 agent 无可用本机 Computer Use surface 运行 macOS native screenshot matrix，故不把该手工 QA 计为通过。Windows T6.3 缺口未改变。
- [x] 2026-08-24：完成 T8.1–T8.4：metadata dialog 采用 viewport-bounded scroll body/fixed actions；播放器 artwork 切换立即 revoke/clear、仅 playing 旋转、native loop、收起唱片 hover/focus overlay、左右箭头和进度左侧播放键；新增 icon title/aria coverage。`rtk vp -C desktop fmt . --check`、check、test（16 files / 57 tests）与 build 均通过；`rtk cargo fmt --all -- --check`、desktop clippy `-D warnings`、desktop all-target tests（56）与 no-bundle Tauri build 通过。Vite dynamic-import/chunk-size 仍为非失败建议。
- [x] 2026-08-24：完成 T7.3–T7.6：详情 Back 改为 sticky pill；toolbar 改为 compact expandable field，clear 内嵌；歌曲行改为单行资料库 grid（序号 hover play、独立 album/format/facts/actions、artist/album filter）；分页右对齐并预留播放器/safe-area 底距。`rtk vp -C desktop test -- HistoryDetail.test.tsx` 通过（14 tests），`rtk vp -C desktop check --no-fmt` 通过。
- [x] 2026-08-24：完成 T7.2。same-ID reset 在 active mutex 内触发单个 SQLite transaction，删除旧 items/orphan artwork 并清零执行事实，再复用统一 conversion pipeline；`rtk cargo test --locked -p unlockm-desktop --lib rerun -- --test-threads=1` 通过（4 tests）。
- [x] 2026-08-24：完成 T7.1 palette 固定尾部 check slot；`rtk vp -C desktop test -- App.test.tsx` 通过（10 tests）。
- [x] 2026-08-24：收到 v0.3.0 使用反馈与 6 张视觉参考，确认 8 项修订。读取 `plan-doc`/`implement-plan`/`review-plan`/`frontend-design`、仓库指导、`docs/PLANS.md`、完整 active ExecPlan、架构与相关 React/Rust/CSS；将重跑语义从“新建 run”改为“same-ID transaction reset”，新增 M7–M8/T7.1–T8.5。此计划更新阶段未修改生产代码。
- [x] 2026-08-24：用户授权提交并发布 `v0.3.0`，同时推送 `origin` 与 `github`。权威版本号开始从 `0.2.1` 升至 `0.3.0`；release tag/workflow 仍须通过全部双平台打包与发布校验。该发布不豁免 T6.3 的 Windows DPI/runtime/跨显示器人工证据缺口。
- [x] 2026-08-24：第二位独立 reviewer 返回 Pass with Notes，确认无剩余代码 findings，10 条 Review Notes 全部 resolved。未请求或创建 commit；计划保留 `In Review`，唯一未完成项为 Windows runtime/DPI/跨显示器 T6.3 QA。
- [x] 2026-08-23：latest remediation 的 `rtk vp -C desktop exec tauri build --debug --no-bundle` exit 0，产物为 `desktop/src-tauri/target/debug/unlockm-desktop`；Vite 仅报告既有 dynamic-import/chunk-size 非失败建议。计划继续 `In Review`，Review Notes 保留独立 reviewer 处理。
- [x] 2026-08-23：主代理对最终 remediation 再复验：`rtk cargo fmt --all -- --check`、`rtk cargo check --locked -p unlockm-desktop`、`rtk cargo clippy --locked -p unlockm-desktop --all-targets -- -D warnings`、`rtk cargo test --locked -p unlockm-desktop --all-targets` 均 exit 0（51 lib tests，另含 license/security integration suites），`rtk git diff --check` 通过。计划维持 `In Review`；Windows DPI/runtime/跨显示器 T6.3 缺口不变，Review Notes 仍由独立 reviewer 关闭。
- [x] 2026-08-23：review remediation：新增 state-owned `IdleHistoryMutationGuard`，metadata 的 editable lookup、封面读取、pending、候选写入、原子发布和 output reconciliation 全程持有同一 guard；delete/clear 复用该 gate。新增 conversion-start/active-run/idle-writer 竞争测试、两个 metadata mutation 的 barrier 串行测试，以及 cover 变更后 returned `HistoryItem.outputBytes` 与已发布文件 `stat` 一致的回归。`rtk cargo fmt --all -- --check`、`rtk cargo check --locked -p unlockm-desktop`、`rtk cargo clippy --locked -p unlockm-desktop --all-targets -- -D warnings`、`rtk cargo test --locked -p unlockm-desktop --all-targets`（55 passed）均通过；Review Notes 保留供独立 reviewer 关闭。
- [x] 2026-08-23：review remediation 已补 copied 筛选、metadata candidate pre-publish verification/FLAC routing、pending reconciliation、播放器 release/restore 与 delete/clear sequencing、whitespace normalization、direct v2→v3 migration、route scroll reset、上传 cover 的即时 artwork item。Review Notes 保留供 reviewer 复验；前端 format/check/test（55 passed）和 diff 通过。
- [x] 2026-08-23：主代理复验 remediation 全绿：root check/clippy、full cargo test（116 unit + 全部 integration/benches）；desktop check/clippy、desktop cargo test（47 lib + license/security）；VP fmt/check/test（16 files/55 tests）/build；Tauri no-bundle exit 0；cargo fmt 与 diff。Vite dynamic-import/chunk-size 仅为非失败建议。

- [x] 2026-08-23：读取 `AGENTS.md`、父级 `RTK.md`、`docs/PLANS.md`、`ARCHITECTURE.md`、CI/release 配置和已完成桌面计划。
- [x] 2026-08-23：检查 React 页面/详情/播放器/筛选/theme、Tauri commands/state/audio protocol、SQLite schema/query/repair、core runner/job/audio/tag pipeline 和现有测试。
- [x] 2026-08-23：确认没有覆盖同一目标的 active plan，创建本 Ready ExecPlan；未修改生产代码。
- [x] 2026-08-23：补充 macOS/Windows 共用的字体、控件和留白密度改进，以及默认 60%、可调/静音/持久化的播放器音量要求；计划保持 Ready。
- [x] 2026-08-23：implementation workflow 已启动；将依照 T1→T6 顺序实现并在每个可验证任务后更新此文档。
- [x] 2026-08-23：M1–M5 实现完成：有界 path probe、copied music 分类/播放、bitrate/schema/preferences、运行级 rerun、新持久播放器/音量、metadata editor/本地封面候选、独立筛选、palette/density 与三语言均已接通；Rust 结果和原生手工矩阵由后续复验记录。
- [x] 2026-08-23：`rtk vp -C desktop check --no-fmt`、`rtk vp -C desktop test`（36 passed）与 `rtk vp -C desktop build` 通过；`rtk cargo fmt --all` 与 `rtk git diff --check` 通过。
- [x] 2026-08-23：补齐 persistent-player 音量回灌/节流保存/显式 mute、旋转封面/reduced motion/object URL 清理；补齐 metadata modal focus trap、候选搜索/lazy artwork、rerun 新文件+旧输出 skip、copied/unknown/symlink/deleted audio protocol、independent filter clear 的测试和实现。
- [x] 2026-08-23：`rtk cargo fmt --all`、`rtk vp -C desktop fmt . --check`、`rtk vp -C desktop check --no-fmt`、`rtk vp -C desktop test`（45 passed）、`rtk vp -C desktop build` 与 `rtk git diff --check` 通过；build 仅报告既有 500 kB chunk/dynamic-import 建议。
- [x] 2026-08-23：修正 `tags.rs` 元数据 patch 的 borrow/match 错误、`job.rs` 未使用导入，以及 desktop history 的类型推导、fixture 歧义转换和 clippy warnings；后续 Rust gates 全部通过。
- [x] 2026-08-23：full cargo test 曾发现 `no_copy_reports_standard_audio_as_skipped_music` 的 skipped 汇总为 0。修复为 source-targeted `SkipMusic`：不 plan/reserve/create output，terminal 仍为 Music/Skipped、保留有界 source snapshot、`destination=None` 以杜绝把源文件当作可编辑输出；回归覆盖 summary、格式、无输出目录和无 destination。
- [x] 2026-08-23：desktop tests 曾发现 legacy copy fixture 未模拟旧 probe version，且 migration v2 分支错误地把 `user_version` 直接写为 v3。fixture 现清除 facts/设 version=0；migration 先提交 version 2，再单独提交 v3 列；回归从 v1 删除全部 v2/v3 列，重开后确认所有列与 version=3 恢复。
- [x] 2026-08-23：主代理复验通过 `rtk cargo check`、root clippy `-D warnings`、`rtk cargo test --locked --all-targets --all-features`（115 unit tests 加全部 integration/benches）、desktop check、desktop clippy `-D warnings`，以及 `rtk cargo test --locked -p unlockm-desktop --all-targets`（44 lib tests 加 license/security）。Tauri no-bundle build 尚待记录。
- [x] 2026-08-23：主代理执行 `rtk /bin/zsh -lc '... vp -C desktop exec tauri build --debug --no-bundle'` 成功（exit 0），生成 `target/debug/unlockm-desktop`。仅有 Vite dynamic-import 和 chunk-size 建议，均为非失败警告。
- [x] 2026-08-23：按产品反馈将 palette 常驻色点改为与语言一致的单 icon 下拉菜单；四项 `menuitemradio` 含色样/本地化文本/Check，支持 roving Arrow/Home/End、Escape/外点关闭及焦点恢复。首页移除 rename，只在详情头部显示该操作。`rtk vp -C desktop fmt . --check`、check、test（46 passed）和 build 通过。
- [x] 2026-08-23：Computer Use macOS QA 发现 WKWebView 不出现 `window.prompt`。移除该依赖，详情标题改为预填、focus/select 的内联 input：Save/Enter 提交、Escape/Cancel 放弃、空白不提交、busy 禁用、失败保持编辑态并经 `onError` 报告。`rtk vp -C desktop fmt . --check`、check、test（47 passed）和 build 通过；build 仅有既有 Vite 非失败建议。
- [x] 2026-08-23：Computer Use metadata modal QA 发现其 Cancel 错误复用“取消转换”。新增中/英/日 neutral `app.cancelEdit` 并只用于 `MetadataEditor`，回归断言英文显示 `Cancel`；format/check/test（47 passed）/build/diff 再次通过。
- [x] 2026-08-23：absolute debug bundle QA 发现内联 rename 同样复用了“取消转换”且编辑时仍显示重复 Pencil。内联编辑现使用 `app.cancelEdit`，且 renaming 时隐藏 Pencil、保留 Reveal；回归断言 neutral Cancel 与没有第二个 rename trigger，format/check/test（47 passed）/build/diff 再次通过。
- [x] 2026-08-23：主代理完成 macOS native QA，使用 `/Users/peilin/Codebase/um-rewrite/unlockm/desktop/src-tauri/target/debug/bundle/macos/UnlockM.app`（未安装 app）。验证：更大且共享的 density；palette 单下拉键盘选择和重启保持；首页无 rename、详情内联 rename 的 focus/select/save/cancel/重启保持且无重复 Pencil；真实临时 MP3 转换；添加文件后重跑（2→30 songs、名称继承）；MP3/65 kbps；主页/详情间持久播放器，默认 0.6、拖到 0.28、mute 0/restore 0.28、重启保持 0.28；metadata 文本保存更新 row/player、neutral modal Cancel、独立 artist clear；长详情固定 Back、滚动出现 Back-to-top 并回到 scroll=0、分页可见。Windows DPI/runtime/跨显示器矩阵仍留 T6.3。

## Discoveries

- 现有全局只有一个 audio 元素，但它实际归 `HistoryDetail` 所有；组件 cleanup 和“活动 item 不在当前页”effect 正是跨页停止的直接原因。
- passthrough 的内容分类在 `formats::prepare()` 已经知道是标准音频，但 `PlannedAction::Copy` 丢失该事实，runner 后续把所有 copy 都标为 companion。
- 有效 MP3 显示 unknown 的高概率原因不是 UI 大小写，而是四个 path consumer 都截取 8192 字节；`detect_audio_type()` 会正确报告需要跳过的 ID3 长度，但 `formats::extension()` 把 `NeedMoreHeader` 折叠为 `None`。
- 当前 tag writer 已具备同目录 candidate、回读校验和 atomic publish，大部分编辑安全能力可复用；主要缺口是显式 clear、ID-only command、history reconciliation 和 UI。
- 当前详情正文存在 `0.58rem`、`0.62rem` 等明显偏小字号；只依赖 DPI 不足以满足可读性，需要同时提高 token 下限。
- 用户确认 macOS 同样存在字体/元素偏小、留白偏多，因此该问题是跨平台视觉基线问题，不应保留为 Windows-only 补丁。
- 原生 `<audio>` 默认音量为 1.0；如果 controller 不在第一次 `play()` 前显式赋值，新播放器可能以 100% 音量启动。
- Tauri 2.11 本地类型和生成 ACL 已包含只读 scale factor/query event 支持，macOS/Windows 界面密度改进不需要引入新插件。
- 当前 metadata writer 的 `Metadata` 模型不能表达“字段保持/清空/set”三态；实现编辑命令前必须补一个明确 patch DTO，不能把空字符串悄悄视作保持原 tag。
- `SkipMusic` 不应沿用 copy 的 output destination：即使最终不写文件，预先规划/保留该 destination 仍会引入不必要的 collision 与输出目录副作用。它应以 source 作为 snapshot target，并在执行前的 destination-equality skip 分支结束。
- Schema migration 每一步都必须写入该步骤的 version；v2 迁移若预先写 v3，会使崩溃后的数据库跳过 v3 列，因而 v1→v3 测试必须实际移除两代列后重开。
- 2026-08-23 CSS 审查：`App.css` 曾被审查指出的 0.62–0.72rem 正文均已提升至至少 0.75rem；当前 App/history/player/modal 的非装饰正文最低为 0.75rem，未保留需要豁免的更小正文。
- macOS WKWebView 不保证 `window.prompt` 提供可用交互；桌面关键写操作应使用 React 控制的内联或 modal UI，而非浏览器 prompt/confirm 弹窗。
- 同一“取消”动作在转换和编辑上下文中语义不同；metadata modal 必须使用 neutral edit-cancel copy，不能复用 conversion cancellation 文案。
- same-ID rerun 刷新了执行时间戳，但 `HistoryStore::list_runs` 使用 `runs.id DESC` 的 ID-keyset 游标；若改成更新时间排序，现有单字段 cursor 将不再稳定并需要复合游标迁移。用户只要求记录数不增加，因此保留 ID 顺序更符合“同一任务位置稳定”的最小变更。
- 原位重跑会删除该 run 的旧 item ID；若播放器仍播放该 run，后续 range 请求会引用已不存在的 item。前端必须在发起 rerun 前通过共享 playback controller 释放匹配 run，和 delete 的安全顺序保持一致。
- 第一轮 M8 实现只有 collapsed overlay opacity transition，expanded content 仍条件卸载，不能形成完整的流畅收展；实现和测试需覆盖容器 layout/content 的进入退出，而不仅是图标替换。
- CSS `inert` 只移除交互，不会移除 grid intrinsic size；mounted 收展内容还必须显式压缩 block axis，否则零宽 grid 会因内部布局换行而保留异常高度。
- `.metadata-modal div` 这类 descendant rule 会覆盖 body/candidate grid 的同等或更低 specificity；dialog layout 只能由命名区域选择器控制，body/actions 增加运行时 layout contract 以防止横向 flex 回归。
- 2026-08-25 截图中的 sticky 贴边来自一个精确 token 脱节：topbar height 与 back button `top` 都是 `68px`。用更大 margin 只能改善初始位置，必须给 sticky offset 加独立 gap 并与 header height 共享来源。
- 当前序号跳号不是 SQLite 丢行或排序错误，而是 UI 显示了持久化 `item.ordinal`；界面序号必须用 page offset + 当前返回数组 index 计算，同时保留 ordinal 作为 server-side 原始顺序键。
- 标题/专辑两列之外的 `.file-facts` 轨道仍带 `0.45fr`，因此它会参与瓜分剩余空间；要实现用户指定的等距右侧列，只有标题/专辑区域可以使用 flexible units。
- 歌手/专辑 button 作为 grid item 默认 stretch，解释了可点击识别区覆盖文字右侧空白；事件冒泡排除只能防止按钮触发行播放，不能缩小按钮自身 hit area。
- 播放器收展入口已是 Phosphor Caret SVG，不是字面文本；问题来自裸按钮几何和 caret 外形仍像 `<>`。封面/覆盖按钮另有 42px/40px 尺寸冲突，expanded overlay 也只有 opacity 隐藏，三者需作为同一视觉状态契约修正。
- M12 的唯一 header handle 只有在 header 与 rows 复用完整 `--music-grid-template` 时才能在 ratio 调整后持续对齐；若各自重复 grid template，后续窄断点/token 变化会重新引入漂移。collapsed state 的 expanded grid track 已为 0，间隔必须由 toggle 的单一 margin 提供，不能恢复父 grid gap。
- T12.4 证明 element-level pointer capture 的调用本身不能作为拖动成功证据：测试必须提供 title/album 的真实 geometry，并断言 move 后 ratio 变化；WKWebView runtime 采用 window drag listeners 后才观察到 `60 → 49`。format 与 facts 即使 token 缩小，若继续分别右/左反向对齐仍会显得过远；改为 format 靠 facts 侧、facts 靠 format 侧，并让 actions 另取稳定 margin 才得到预期视觉分组。
- CSS custom property 的依赖方向同样是布局 correctness 边界：父元素的 `--music-grid-template` 在父级 compute 时不能读取仅定义于后代的 ratio variables；必须把 ratio 和 template 放到同一元素，或在每个 grid element 完整定义 template。AX `aria-valuenow` 改变只能证明 React state/ARIA 改变，不能替代实际 grid geometry 证据。

## Decision Log

- **2026-08-23 — D1:** 重跑创建新历史记录并重新扫描，复制旧任务 name/path/settings，不覆盖或改写旧任务；`overwrite` 继续为 false。
- **2026-08-23 — D2:** 任务展示名入 SQLite；旧行使用输入 basename fallback，避免一次性路径字符串 SQL backfill。
- **2026-08-23 — D3:** 标准音频属于 music，与 action/outcome 正交；`copied` 是可播放、可编辑的成功 music outcome。
- **2026-08-23 — D4:** MP3 修复采用集中 path-based checked seek probe，不用扩展名兜底，也不把整个 ID3 artwork 读入内存。
- **2026-08-23 — D5:** 播放器 controller 挂在页面切换之外；列表行只负责选择/切换歌曲，不拥有播放进度。
- **2026-08-23 — D6:** 元数据编辑只改已验证输出，封面上传传 bytes 而非路径；已有封面搜索仅限本地历史，不新增网络服务。
- **2026-08-23 — D7:** 用 pending marker + 磁盘回读修复文件/SQLite 两阶段中断；原子文件安全优先于伪造的跨资源事务。
- **2026-08-23 — D8:** “独立清空”适用于 search/artist/album/outcome；sort 是排序而非筛选，不随任何清空动作重置。
- **2026-08-23 — D9:** 配色是独立于 light/dark 的 palette 轴，首批提供 blue/emerald/violet/amber，旧 preference 默认 blue。
- **2026-08-23 — D10:** macOS/Windows 共用更大的字体、图标和控件基线并减少无效留白；原生缩放率只温和校准 density tokens，不对 Retina/WebView2 做整页二次 zoom。
- **2026-08-23 — D11:** 播放器缺失/非法音量 preference 时默认 0.6，并在第一次 `play()` 前应用；持久化范围为 0.0–1.0，绝不静默回退到浏览器默认 1.0。
- **2026-08-23 — D12:** 展开播放器提供音量 range 和静音/恢复；静音恢复最近正音量或 0.6，显式 volume 0 可跨重启保持。
- **2026-08-23 — D13:** 历史 schema 升级为 v3，使用 nullable `display_name` 的读取 fallback，并将新音频事实字段保留为 nullable/默认值以允许旧库渐进修复。
- **2026-08-23 — D14:** metadata edit 使用 ID-only `MetadataPatch`（`Option<Option<String>>`）表示不变/set/clear；候选文件仍由既有同目录 `prepare_write`/`AtomicOutput` 边界负责，history 在候选发布前写 pending 并在后续详情读取时基于磁盘事实清除。
- **2026-08-23 — D15:** palette 改为与语言选择一致的单一可访问下拉菜单；每个 palette option 显示色样、本地化名称和选中标识，支持 keyboard roving focus、Escape/外点关闭与焦点恢复。
- **2026-08-23 — D16:** 重命名入口只显示在详情页的任务头部；首页只保留详情、重跑、输出和删除等列表级操作，避免首页操作密度过高。
- **2026-08-23 — D17:** macOS WKWebView 不依赖 `window.prompt`；详情页标题内联完成重命名，自动 focus/select，Enter 保存、Escape/Cancel 放弃，空白值不提交，失败保持编辑状态并显示既有错误。
- **2026-08-23 — D18:** metadata、delete 和 clear 必须通过 state-owned idle mutation guard 与 conversion start 互斥；guard 覆盖候选写入、文件发布及 SQLite reconciliation，避免 disk/history 的跨请求交错。
- **2026-08-24 — D19:** 发布版本定为 `0.3.0`，使用现有 checksum-backed unsigned-preview tag workflow；同一 reviewed commit 与 annotated `v0.3.0` tag 推送到 `origin`/`github`。Windows CI 的 build/package/PE-subsystem 证据可以记录为发布证据，但不能替代 T6.3 的 Windows DPI/runtime/跨显示器人工 QA。
- **2026-08-24 — D20（取代 D1 的产品语义）:** 重跑不再创建新历史记录。它保留同一 run ID、display name 和 Rust-owned request snapshot，以 transaction reset 删除旧 item/counters/error/representative artwork 后复用统一 pipeline 写入最新事实；历史任务总数保持不变，输出文件仍遵守 `overwrite = false`。
- **2026-08-24 — D21:** palette menu 的每项永久包含 `swatch | label | check-slot` 三列；未选中只隐藏 check glyph，不移除尾列，避免文字移动。
- **2026-08-24 — D22:** 筛选使用 compact-expandable 控件而不是整块 filter drawer：空值 blur 时坍缩，focus 或有值时展开，clear 固定在控件内部；这样保留一键访问同时减少工具栏占用。
- **2026-08-24 — D23:** 歌曲列表采用媒体资料库式单行 grid。序号按钮是键盘主播放入口，整行非交互区域提供 pointer shortcut，歌手/专辑和三项文件操作保持独立可点击边界。
- **2026-08-24 — D24:** 元数据候选采用 viewport-bounded 内部滚动而非新增后端分页协议；现有 bounded 12-item 查询保持不变，dialog header/actions 固定可达。
- **2026-08-24 — D25:** 播放器使用单曲 native loop；圆形封面仅在真实播放时旋转，收起态唱片优先、hover/focus 才显示播放控制，左右箭头表达收展方向。
- **2026-08-24 — D26:** same-ID rerun 不改变历史 ID-keyset 排序或 cursor 契约。时间戳表示最新执行事实，但任务保留原列表相对位置；若未来产品要求“最近重跑置顶”，应另行设计 `(updated_at_ms, id)` 复合游标，而不是在本修订中暗改分页。
- **2026-08-24 — D27:** 发起 same-ID rerun 前 UI 释放该 run 的活动播放器 source，不尝试把旧 item ID 恢复到新明细；播放器的收展必须具有真实 layout/content 动画，并为 expanded 状态移除不可见的重复播放按钮焦点。
- **2026-08-24 — D28:** 本轮补丁版本为 `0.3.1`。`v0.3.1` 只创建在本地，远程仅接收 reviewed `main` commit；不得执行 `git push --tags`、`git push <remote> v0.3.1` 或触发 tag release workflow。macOS 产物仅本地构建、签名和安装。
- **2026-08-25 — D29:** v0.3.1 截图反馈继续更新本 active plan，不新建重复 UI plan。由于新增 M9–M11 尚未实现，整体状态从 `In Review` 重开为 `Ready`；旧 M7/M8 review evidence 保留但不自动覆盖新任务。
- **2026-08-25 — D30:** 详情主页入口的可见文案为 `返回主页 / Home / ホーム`；sticky offset 由共享 topbar height 加独立 spacing token 组成，不能再用两个相等的 `68px` 硬编码。
- **2026-08-25 — D31（按用户更正）:** 列表序号表示当前筛选/排序结果中的分页位置，计算为 `(page - 1) * pageSize + visibleIndex + 1`；完整结果从 1 开始并跨页连续，例如每页 10 条时第二页从 11 开始。展示序号不持久化，持久化 ordinal 和后端 query 不变。
- **2026-08-25 — D32:** 标题/专辑是唯一可拖动的 flexible region，比例只在当前详情会话中保持并 clamp；右侧 format/facts/actions 为所有行共享的稳定 fixed CSS token tracks，且以统一 gap 等距排列。separator 必须同时支持 pointer capture、键盘和 ARIA，窄断点隐藏。
- **2026-08-25 — D33:** 歌手/专辑筛选 hit area 只覆盖可见文本和必要 focus padding；列内空白属于 row playback。collapsed cover/overlay 共享单一尺寸 token，expanded overlay 使用 opacity+visibility+pointer/focus 强隐藏，收展按钮改用居中的 `ArrowLineLeft/ArrowLineRight` 类方向 icon。
- **2026-08-25 — D34（覆盖 D32 的 handle placement）:** 标题/专辑继续是唯一 flexible region，右侧继续使用 shared fixed token tracks；但不再在每一歌曲行渲染 separator。唯一可访问、可拖动的 handle 位于与 rows 共同 grid tracks 的 column header，行内仅保留非视觉 spacer。折叠播放器使用 toggle margin（或等价的不双算 geometry）在 disc 与 expand button 之间保留约 8px 间隔。
- **2026-08-25 — D35（真实运行反馈覆盖 M12 初次验收）:** header 不再作为结果计数下方的额外独立行；计数/companion 与全部列标题必须组成单一共同 grid 行。单元素 pointer capture 不足以视为原生拖动通过，拖动生命周期须在 WKWebView 中可靠接收 move/up/cancel 并以实际 ratio 变化证明。artist/album 以真实有值 metadata 逐项验证筛选 query 与 row-playback 隔离。format 与 facts 靠近，facts 与 actions 用独立 gap 分开；此前 collapsed 8px 方案保持。
- **2026-08-25 — D36（覆盖 D35 的 visual-layout 细节）:** ratio CSS variables 与 `--music-grid-template` 共同归 `.song-list` 所有，避免父级 template 在 ratio 尚不存在时永久 fallback。facts 标题为 `大小与码率 / Size & bitrate / サイズとビットレート`；format—facts 与 facts—actions 使用同一 uniform grid gap，不用 actions 的额外 margin。format 向 facts 对齐、facts 在 fixed track 内居中、actions 靠左，使 facts 两侧的视觉余量一致。

## Completion Evidence

- 2026-08-25：T12.5 最终闭环由三层独立证据组成：实现代理的 absolute-bundle pointer drag（line x≈485→385、AX 60→41），原 M12 reviewer 对 CSS custom-property 同作用域修复的 re-review `Pass`（28 focused tests/check/diff），以及主代理的第二次 absolute-bundle drag（line x≈320→448、AX 41→75）与全量 VP 71 tests/build。三方均验证 `.song-list` 统一拥有 ratio/template，header/rows 随之重排；旧 T12.4 “AX 变化即视觉通过”的结论已 superseded。Windows T6.3 与真实超长 metadata 极值截图仍是明确外部/非阻断缺口，因此计划保持 `In Review`。
- 2026-08-25：T12.5 final visual evidence：source/test gates 为 focused HistoryDetail/locale 26 tests、VP full 17 files / 71 tests、fmt/check/build/diff all pass；Vite 仍只有既有 dynamic-import/chunk-size warnings。重建后通过 `pgrep` 确认旧 workspace debug PID 并关闭，再由 Computer Use 只启动 absolute workspace bundle `/Users/peilin/Codebase/um-rewrite/unlockm/target/debug/bundle/macos/UnlockM.app`，避免复用重建前 memory image。1080×760 screenshot 中 default separator x≈485，拖动后实际位于 x≈385（AX 60→41），title/album columns visibly resize；920×600 screenshot 显示 `大小与码率`、操作列，format—facts 和 facts—actions 使用一致间隔。未使用 `/Applications`；Windows T6.3 和真实超长 metadata/pointer 极值仍开放。
- 2026-08-25：T12.4 的事件/筛选 native evidence 仍有效，但其视觉 drag 和 trailing-gap 结论已由 T12.5 supersede：AX value 变化并未证明 CSS template 实际依赖 ratio，facts/actions 的 margin 也不符合最新等距要求。T12.5 必须以 parent-owned template 与 native separator x-coordinate 前后变化、两侧间距截图重新提供 evidence。
- 2026-08-25：T12.4 runtime evidence 取代被标记 superseded 的 M12 初次通过结论。自动化为 focused HistoryDetail/locale 26 tests 和完整 VP 17 files / 71 tests，fmt/check/build/diff 全部通过；Vite 仅保留既有 dynamic-import/chunk-size 非失败建议。Computer Use 只使用 `/Users/peilin/Codebase/um-rewrite/unlockm/target/debug/bundle/macos/UnlockM.app`：在 1080×760 实拖唯一细线 handle，AX `aria-valuenow` 由 60 变为 49；用真实 QA Track 顺次点击 QA Artist/QA Album，artist、album filter 输入均填入对应值且结果为 1 首；920×600 截图确认单行 count/columns/actions、紧凑 trailing tracks、事实到操作留白和 collapsed 8px。未启动 `/Applications`，Windows T6.3 与真实超长 metadata/pointer 极值仍是明确跟进缺口。
- 2026-08-25：M12 的先前 source/automated/initial macOS evidence 已被用户真实运行反馈部分推翻，不能再作为 T12.4 的通过证据。它仅保留为初始实现的历史记录；T12.4 必须用真正改变 ratio 的 native drag、真实 artist/album filter 及重做的单行 header/trailing spacing QA 替代。
- 2026-08-25：M12 完成 source、自动化和 macOS native evidence。定向 HistoryDetail/PersistentPlayer/locale tests 为 27 passed，完整 VP gates 为 17 files / 70 tests；`rtk vp -C desktop exec tauri build --debug --bundles app` 生成 absolute workspace bundle。Computer Use 只针对 `/Users/peilin/Codebase/um-rewrite/unlockm/target/debug/bundle/macos/UnlockM.app` 完成 1080×760 和 920×600 验收：一个表头 handle、无行内 separator/tab stop、表头与行的共享列对齐、完整中/英/日列标题、收紧的右侧 tracks 与折叠播放器约 8px 间隔；未启动或读取 `/Applications/UnlockM.app`。Windows T6.3 和真实超长 metadata/pointer ratio 极值仍是明确后续缺口。
- 2026-08-25：主代理独立复验本轮最终工作树：`rtk vp -C desktop fmt . --check`（58 files）、`rtk vp -C desktop check --no-fmt`（40 files clean）、`rtk vp -C desktop test`（17 files / 69 tests）、`rtk vp -C desktop build` 与 `rtk git diff --check` 均通过；build 仅保留既有 ineffective dynamic import 和大于 500 kB chunk 建议。Computer Use 只启动绝对工作区 bundle `/Users/peilin/Codebase/um-rewrite/unlockm/target/debug/bundle/macos/UnlockM.app`，未使用 `/Applications`：独立截图确认日文三个 placeholder 完整、Home 与顶栏约 10px 间隔、第二页编号连续 `26`–`30`、expanded 无封面 overlay、collapsed disc/control 对齐且音量为 `0.6`。Windows T6.3 和真实超长 metadata/pointer 极值 native 跟进仍开放。
- 2026-08-25：M9–M11 的 macOS native QA 使用的唯一应用是当前工作区 absolute debug bundle `/Users/peilin/Codebase/um-rewrite/unlockm/target/debug/bundle/macos/UnlockM.app`，未混用 `/Applications/UnlockM.app`。实际 30 首 fixture 在 1080×760 和 920×600 下完成中/英/日 compact placeholder、sticky 入口、连续序号、分页、M10 default grid 与 M11 expanded/collapsed player 的截图检查；自动化 VP gates 为 69 tests。未在这个 fixture 中重建真实超长 metadata/pointer ratio 极值截图，debug bundle 也不是 release 签名认证产物；Windows T6.3 原生 DPI/runtime/跨显示器矩阵仍未完成。因此计划只能 `In Review`，不能标记 `Done`。
- 2026-08-24：M7/M8 独立 review 最终为 Pass with Notes、无未闭环代码 finding；same-ID failure/cancel/event ID 与 cover frozen-angle 两条 Minor 已由原 reviewer 复验关闭。当时仅 Windows T6.3 仍开放，因此计划保持 `In Review`；2026-08-25 新增 M9–M11 后该结论不再代表全部当前 scope。
- 2026-08-24：M7–M8 implementation 与两轮 native-QA remediation 已完成；主代理完整 Rust/VP gates、macOS debug app bundle build 和 absolute-bundle Computer Use 矩阵均通过。用户给出的 8 项修订在 macOS 1080×760 与 920×600 场景完成可见验证；未创建 commit/tag/push。
- 2026-08-24：same-ID rerun 在真实 30 首任务上返回同一记录，首页始终显示 `1 records`；输出仍按 no-overwrite skipped，详情保持 30 首。真实 metadata 上传把 QA MP3 从 1.8 KB 更新到 63.6 KB 并立即展示圆形封面；随后切到无封面曲目时播放器改为当前占位而非保留旧图。1 秒音频经过自然时长仍保持播放，验证 native loop。
- 2026-08-24：Windows 100/125/150/200% DPI、跨显示器移动和本轮新增布局/播放器/metadata runtime QA 仍并入既有 T6.3 外部缺口；此缺口不否定 macOS 本轮验收，但计划在独立 review 后仍不会标记 `Done`。
- 2026-08-24：第二位独立 code review 对当时的 M7/M8 结论为 Pass with Notes；无未闭环代码 findings，旧 Review Notes 全部 resolved。未创建 commit；当时 T6.3 Windows native QA 是保持 `In Review` 的唯一原因，该 review 不覆盖后来新增的 M9–M11。
- 2026-08-23：latest remediation 的 macOS no-bundle Tauri debug build exit 0，产物 `desktop/src-tauri/target/debug/unlockm-desktop`；Vite dynamic-import/chunk-size 为既有非失败建议。
- 2026-08-23：主代理最终复验通过 desktop Rust fmt/check/clippy `-D warnings`/all-target tests（51 lib tests + license/security integration suites）及 `git diff --check`；当时计划状态为 `In Review`。Windows 100/125/150/200%、跨 DPI display 与 runtime QA 是该版本的 T6.3 平台验证缺口，Review Notes 未由 implementer 勾选。
- 2026-08-23：未创建 commit（按请求保留未提交工作树）。前端 format/check/test/build 和 diff 检查通过；`vp test` 共 47 项。
- 2026-08-23：主代理复验已通过 root `cargo check`、root clippy `-D warnings`、full `cargo test --all-targets --all-features`（115 unit + 全部 integration/benches）、desktop check、desktop clippy `-D warnings`、`cargo test -p unlockm-desktop --all-targets`（44 lib + license/security），以及 macOS `vp -C desktop exec tauri build --debug --no-bundle`（exit 0，产物 `target/debug/unlockm-desktop`）。Vite dynamic-import/chunk-size 仅为非失败建议。
- 2026-08-23：review remediation 后，主代理再次通过 root check/clippy/full test（116 unit + integration/benches）、desktop check/clippy/test（47 lib + license/security）、VP fmt/check/test（16 files/55 tests）/build、Tauri no-bundle（exit 0）及 cargo fmt/diff；Vite dynamic-import/chunk-size 仅为非失败建议。Review Notes 仍等待独立 reviewer 关闭。
- 2026-08-23：macOS absolute debug bundle QA 已完成（`desktop/src-tauri/target/debug/bundle/macos/UnlockM.app`）；因没有已安装 app，验证直接针对该 bundle。上述 conversion/rerun/rename/player/metadata/filter/navigation/palette/density 行为均通过。Windows 100/125/150/200%、跨 DPI display 与 runtime QA 仍为唯一 T6.3 缺口；T6.4 由独立 review agent 执行。

## Review Notes

- [x] Major — Resolved on M9 re-review: 空值 compact artist/album 输入现在仅在 `data-has-clear=true` 时预留 `padding-inline-end: 34px`，三语言 placeholder/clear-slot contract 已覆盖。后续 T11.3 已完成 M9 的三语言 native screenshot 与 920×600 验收；本条的“尚未完成”历史状态由 Completion Evidence 2026-08-25 最终复验取代。
- [x] Review — Superseded: M9–M11 已实施；既有 M7/M8 Pass with Notes 不得视为对本轮新改动的通过。M9/M10/M11 分别保留其独立审查记录与 T11.3 集成证据。
- [x] Major — Resolved on re-review: `copied` is now present in Rust/TypeScript filters and the detail selector, with copied-filter UI coverage.
- [x] Major — Resolved on re-review: metadata candidates verify text/cover/container/audio before publish and route FLAC through the dedicated writer; MP3/FLAC payload regressions cover set/clear/cover behavior.
- [x] Major — Resolved on re-review: pending recovery transactionally reconciles text, artwork, representative artwork, bytes, format, bitrate, and probe version before clearing the marker; missing outputs remain pending.
- [x] Major — Resolved on re-review: metadata edits release/reload the active audio source and delete/clear release matching playback first, with ordering tests.
- [x] Minor — Resolved on re-review: Rust normalizes whitespace-only fields to clears and trims set values.
- [x] Minor — Resolved on re-review: metadata command returns the authoritative `HistoryItem`, including the inserted artwork ID, to the immediate UI/player state.
- [x] Minor — Resolved on re-review: direct v2-to-v3 migration coverage exists, remaining text is at least 0.75rem, and home/detail transitions reset scroll.
- [x] Minor — Resolved on re-review: route scroll reset is covered without coupling it to playback state.
- [x] Major — Resolved on second re-review: normal metadata publish calls authoritative output reconciliation, transactionally returning text/duration/artwork/representative-artwork/bytes/format/bitrate/probe facts; a cover-size-changing command test verifies the returned byte count.
- [x] Major — Resolved on second re-review: `IdleHistoryMutationGuard` holds the idle active-run lock across lookup, candidate creation, publication, and reconciliation, and is shared with conversion start/delete/clear; state and command contention tests verify the ordering.
- [x] Review — Second independent code re-review passed. Windows runtime/DPI and cross-monitor QA remains the explicit external T6.3 validation gap; it is not inferred from macOS evidence.
- [x] Minor — Resolved on re-review: deterministic same-ID scan-failure and cancellation tests now retain the original terminal ID, replace old facts, release the active slot, and assert captured camel-case event `runId` values (including `completed.cancelled=true`).
- [x] Minor — Resolved on re-review: cover animation is installed permanently and switches only `animation-play-state` between running and paused, preserving the current transform when playback pauses; focused player tests cover both states.
- [x] Review — 2026-08-25 M10 remediation re-review: **Pass**. All prior M10 findings are resolved: every row has the same fixed tail tracks and shared flexible title/album ratio; long/unknown metadata preserves intrinsic text hit targets and ellipsis; separator pointer capture is released on up/cancel and cleared on lost capture; the 820px media-query path omits the separator and tab stop. Independently ran `rtk vp -C desktop test src/features/history/HistoryDetail.test.tsx` (1 file / 21 tests), `rtk vp -C desktop check --no-fmt` (40 files clean), and `rtk git diff --check` (pass). This was a chronological review note: M9 T11.3 and M11 review subsequently completed. Real long-metadata/pointer-extreme native checks remain the documented non-blocking follow-up.
- [x] Review — 2026-08-25 M11 code review: **Pass**. Expanded-state collapsed overlay is disabled, removed from the tab order and accessibility tree, and CSS-hidden/non-hit-testable; collapsed state uses the same `--player-disc-size` circular geometry for artwork and overlay. The dedicated 42px `ArrowLineLeft`/`ArrowLineRight` control inherits centered icon-button geometry while preserving state-specific aria-label/title. Existing provider coverage continues to assert native loop and safe volume; player coverage asserts running-to-paused rotation and the expanded/collapsed DOM contract. Independently ran `rtk vp -C desktop test src/features/playback/PersistentPlayer.test.tsx` (1 file / 2 tests), `rtk vp -C desktop check --no-fmt` (40 files clean), and `rtk git diff --check` (pass). The macOS T11.3 evidence is recorded by the implementer; this code review did not independently reproduce native UI QA. Windows T6.3 remains open.
- [x] Review — 2026-08-25 M12 independent review: **Pass, superseded by runtime evidence**. The static implementation correctly had one wide-layout `role="separator"`, shared tracks and a collapsed 8px margin, but user-native feedback found an extra header row, non-working actual drag, failed real artist/album filtering and unsatisfactory trailing alignment. This code review remains chronological evidence only; T12.4 must receive a new independent review after runtime remediation.
- [x] Review — 2026-08-25 M12 runtime re-review: **Pass, superseded by T12.5 visual-layout evidence**. T12.4 correctly fixed events and filtering, but review did not detect that the parent-owned `--music-grid-template` had already resolved fallback ratio values before descendants supplied inline variables; its AX ratio and trailing-margin conclusions therefore cannot approve actual visual drag/spacing. T12.5 requires a fresh independent review after parent-owned template and native coordinate evidence.
- [x] Review — 2026-08-25 M12 T12.5 re-review: **Pass**. The ratio variables and the `--music-grid-template` that consumes them now share `.song-list`; header and rows inherit the same resolved tracks and carry no local ratio overrides. Focused regressions assert parent `40fr/60fr` then `70fr/30fr`, empty child styles, geometry-based window drag lifecycle and pointer release. All three facts labels are restored as `大小与码率 / Size & bitrate / サイズとビットレート`. The shared grid gap is uniformly 12px, with format right-aligned toward a centered facts track and actions left-aligned without an additional margin, so header and rows use the same trailing geometry. Count/header/Actions, populated artist/album isolation, unknown noninteractive values, the narrow breakpoint and collapsed player’s single 8px margin remain intact. Independently ran `rtk vp -C desktop test src/features/history/HistoryDetail.test.tsx src/i18n/locales.test.ts src/features/playback/PersistentPlayer.test.tsx` (3 files / 28 tests), `rtk vp -C desktop check --no-fmt` (40 files clean), and `rtk git diff --check` (pass). T12.5’s fresh absolute-workspace native coordinate evidence correctly supersedes T12.4 visual claims; Windows T6.3 and real long-metadata/pointer-extreme native follow-ups remain open.
- [x] Review — 2026-08-25 final M9–M12 holistic review: **Pass**. The complete uncommitted source, tests, locales, CSS, untracked `layout.ts`, and current plan agree with the user-visible contract: page-relative numbering is `(page - 1) * pageSize + index + 1`; the single header separator updates the parent-owned ratio/template that every row inherits; count and localized headers share one row; artist/album controls remain intrinsic and isolated from row playback; fixed format/facts/actions tracks use one 12px grid gap; and the player has a true expanded-overlay exclusion plus a single collapsed 8px disc/toggle gap and centered direction icons. The review found no unresolved code or plan finding. Independently ran `rtk vp -C desktop test` (17 files / 71 tests), `rtk vp -C desktop check --no-fmt` (40 files clean), and `rtk git diff --check` (pass). Existing Windows T6.3 and real long-metadata/pointer-extreme native checks remain explicitly non-blocking external validation gaps, so overall plan status correctly remains `In Review`.
