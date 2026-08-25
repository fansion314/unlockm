# Multicolor Theme System and Youthful Visual Refresh

## Status

In Review

Last updated: 2026-08-26

## Objective

将 UnlockM 桌面端从“中性表面 + 单一强调色”的朴素界面升级为更年轻、更贴近音乐产品气质的完整视觉系统，同时保持信息层级、可读性、跨平台一致性和现有交互语义。

用户完成升级后应看到两个直接结果：

1. 首页、历史详情、播放器和元数据编辑器使用协调的多色环境渐变、层次化表面、状态色和细腻微交互，不再让大面积区域呈现无色、割裂或只有主按钮着色的观感。
2. 外观设置不再是 `light/dark × blue/emerald/violet/amber` 的简单强调色组合，而是类似 IDE 的完整主题预设。每个预设独立定义背景、卡片、文字、边界、三组品牌色、状态色、渐变、阴影和 `color-scheme`；首批包含 Tokyo Night 以及多套同等级经典主题。

技术结果是建立一个可扩展、可验证、兼容旧偏好的 `ThemeId`/主题注册表边界，让新增主题不需要继续在组件 CSS 中追加局部覆盖，并让主题完整性、对比度、持久化迁移和 reduced-motion 行为可被自动化测试。

## Scope

- 为桌面端建立完整语义主题 token 契约，覆盖 canvas、ambient gradient、header、card、raised/control/hover/selected/active surface、主要/次要/第三强调色、正文层级、边界、focus、成功/警告/危险、遮罩、骨架屏和阴影。
- 首批提供 9 个完整主题预设：原创 `UnlockM Aurora Light`（新安装默认）、`UnlockM Aurora Dark`，以及 `Tokyo Night`、`Catppuccin Mocha`、`Dracula`、`Nord`、`Gruvbox Light`、`Gruvbox Dark`、`Rosé Pine Dawn`。
- 每个预设至少包含三种非状态用途的协调色，并以主题自身的 light/dark `color-scheme`、表面层级、文字色和渐变工作；不再把经典主题解释为只替换 `--color-accent`。
- 将 Rust/TypeScript preferences 收敛为一个 canonical `themeId`，兼容读取既有 `theme` + `palette` JSON 和 `unlockm-theme`/`unlockm-palette` localStorage，并在后续保存时写回新格式。
- 将顶栏现有 palette 菜单和 light/dark 切换合并为一个外观/主题选择器，提供主题名称、light/dark 标识、多色预览、当前选中状态、键盘导航、Escape/外点关闭、焦点恢复和窄窗口滚动。
- 重塑全局画布、顶栏、首页 composer、拖放态、设置区、转换 CTA、历史列表、详情头、筛选器、歌曲列表、分页、播放器、错误提示、元数据弹窗和在线元数据区域的配色、渐变、边框、阴影、hover/press/selected/focus 状态。
- 保持现有字体、信息架构、路由、窗口密度、列表列布局、sticky/fixed 位置、播放器能力、元数据写入边界和 Tauri 权限模型。
- 为中/英/日补充外观选择与主题说明文案，更新契约 fixture、架构文档、第三方主题来源/许可声明、自动化测试和 macOS/Windows 原生视觉 QA 记录。

## Non-Goals

- 不修改 CLI、转换核心、扫描/watch、格式 adapter、路径规划、元数据合并、输出发布或网络 provider 行为。
- 不新增用户自定义调色器、主题 JSON 导入/导出、云端主题市场、账号同步、定时切换或跟随系统主题；本轮只支持内置完整预设和显式用户选择。
- 不为每个主题更换字体、图标集、布局密度、封面形状或组件结构；主题改变视觉语义，不改变功能和内容优先级。
- 不把界面做成持续运动的“音乐可视化”。装饰性背景保持静态；连续动画仍仅用于已有的播放中唱片旋转和运行/骨架状态，并受 reduced-motion 约束。
- 不引入新的图片素材、远程资源、CSS 框架或组件库；本计划使用现有 React、Motion、Phosphor、字体和 CSS custom properties。
- 不在主题实现中放宽 webview CSP、Tauri capability、文件系统、网络、opener 或音频协议权限。
- 不顺带重排详情表格、重写播放器状态、改变 metadata editor 草稿/保存语义，或关闭现有计划中的 Windows QA、在线 provider 外部网络等独立缺口。

## Current Context

- 计划基线为提交 `faac4db1087eb05ccc8462da7be0ffef133fb76f`，创建计划时工作树干净。
- `docs/plans/2026-08-23-desktop-history-player-metadata-ux.md` 仍为 `In Review`，已经实现 palette、播放器、元数据弹窗和密度基线，只剩独立 Windows 原生 QA；本计划依赖其当前实现，不吸收或声称关闭该缺口。
- `docs/plans/2026-08-25-online-metadata-search-spotify.md` 仍为 `In Review`，已经扩展 `MetadataEditor.tsx` 和 `.metadata-online`。本计划只统一该区域的视觉 token/状态，不改 provider、网络和草稿语义；实施前须以当时最新代码为基线处理同文件重叠。
- `desktop/src/styles/tokens.css` 当前在 `:root` 与 `[data-theme="dark"]` 中定义完整 light/dark 中性色；`emerald`、`violet`、`amber` selector 只覆盖 `--color-accent`、hover、focus 和 border accent，因此 8 种组合仍共享绝大多数表面、文字、状态、阴影和交互色。
- `desktop/src/lib/contracts.ts`、`desktop/src-tauri/src/history/mod.rs::Preferences` 当前暴露 `theme: light | dark` 与 `palette: blue | emerald | violet | amber`。SQLite 的 `preferences` 表保存单条 JSON，不需要 schema migration，但直接更换字段会使旧 JSON 无法读取，必须提供兼容 decoder。
- `desktop/src/lib/theme.ts` 使用 `unlockm-theme` 与 `unlockm-palette` localStorage，`main.tsx` 在 React mount 前调用 `applyTheme(cachedTheme(), cachedPalette())`，`App.tsx` 在 bootstrap 后以 Rust preference 再次对齐。新模型必须保留这条“缓存预绘制、Rust 最终权威”的顺序，避免启动闪烁和前端缓存覆盖持久偏好。
- `desktop/src/components/AppHeader.tsx` 当前有独立 Palette 菜单和 Moon/Sun 按钮，已经实现 roving focus、Escape、外点关闭和 trigger focus restore；这些可访问性模式应复用于单一主题选择器。
- `desktop/src/App.css`、`styles/history.css`、`styles/player.css` 已普遍消费语义变量，但仍有 palette swatch 固定 hex、黑色 backdrop/shadow、局部 `color-mix`，以及 `.metadata-online` 引用未定义的 `--line`。视觉重构应把这些全部纳入主题契约，避免主题外颜色泄漏或 fallback 失效。
- `App.css` 负责顶栏、composer、历史列表、运行状态、错误提示和响应式断点；`history.css` 负责详情、筛选、歌曲表格、元数据弹窗和在线元数据；`player.css` 负责固定播放器。主题刷新会横跨三者，但不得破坏 920px/820px/720px/680px 现有断点和 fixed/sticky 几何。
- `desktop/src/app/page-motion.ts` 与 CSS 已有 160–260ms 的统一过渡和 `prefers-reduced-motion` 分支；新 hover、主题切换和环境层次应沿用相同 motion token，不另造高频或无法中断的动画。
- `desktop/src/App.test.tsx` 已覆盖 light/dark、palette persistence 和菜单键盘交互；`lib/theme.test.ts`、`preferences.test.ts`、`contracts.test.ts` 与 Rust history/fixture tests 覆盖当前偏好契约。它们是迁移回归的主要落点。
- CI 使用 Vite+ 的 format/check/test/build，并在 macOS/Windows 编译桌面端；仓库没有现成 screenshot regression harness，因此自动化负责主题契约、迁移、对比度和交互，最终视觉质量仍需原生截图/手工矩阵。

## Design and Decisions

### 1. 完整主题预设取代组合式 palette

- 新的 canonical 类型命名为 `ThemeId`，首批稳定 ID 为 `aurora-light`、`aurora-dark`、`tokyo-night`、`catppuccin-mocha`、`dracula`、`nord`、`gruvbox-light`、`gruvbox-dark`、`rose-pine-dawn`。显示名称来自 i18n/registry，持久化只保存稳定 ID。
- `desktop/src/lib/theme-presets.ts` 作为前端主题 registry，至少保存 ID、`light | dark` scheme、显示/说明 key、4 色预览和分类；组件不得自行维护第二份主题列表或硬编码 swatch。
- `desktop/src/styles/tokens.css` 保留字体、间距、圆角、密度、motion 等结构 token；新建 `desktop/src/styles/themes.css` 保存每个主题的完整颜色 token block。`tokens.css` 不再承载 `data-palette` 分支，组件 CSS 不直接根据主题 ID 分叉。
- 每个主题 block 必须显式提供同一组 required tokens。允许用 `color-mix()` 产生轻微 hover/透明层，但不能依赖另一主题 selector 的颜色值；`Aurora Light` 的 `:root` fallback 与显式 selector 必须一致。
- 颜色 token 分成以下稳定角色：
  - canvas/ambient：`--surface-app`、`--surface-app-alt`、三组 ambient、`--gradient-app`、`--gradient-hero`、`--gradient-action`；
  - surfaces：header、card、card-tinted、raised、soft、control、hover、selected、active、danger、success、skeleton 及 highlight；
  - brand：primary/hover、secondary、tertiary、on-accent、focus、accent border；
  - content：primary、secondary、muted、disabled、on-strong-surface；
  - structure/state：subtle/control/strong border，danger/success/warning，modal backdrop，menu/floating/colored shadow。
- 实施时对 token 名做一次最终归并，删除同义或未消费角色；但上述能力不得退回“只改 accent”的实现。

### 2. 首批主题目录与来源政策

- `UnlockM Aurora Light` 是新安装默认：浅色暖白/淡蓝紫 canvas，蓝紫 primary、青绿 secondary、珊瑚/粉橙 tertiary，强调音乐感但保留正文的中性可读区。
- `UnlockM Aurora Dark` 是原创深色搭配：深靛蓝 canvas，电蓝/紫/青高光，作为不依赖第三方名称的深色选择。
- 经典主题提供完整表面与多色解释：Tokyo Night（蓝/紫/青）、Catppuccin Mocha（mauve/blue/peach）、Dracula（purple/cyan/pink）、Nord（frost/aurora）、Gruvbox Light/Dark（aqua/orange/red）、Rosé Pine Dawn（rose/foam/gold）。
- 经典主题只复用/转译颜色体系和显示名称，不导入上游代码或运行时依赖。实施前从各主题官方上游锁定 palette 参考、版本/commit 与许可证；兼容时在 `THIRD_PARTY_NOTICES.md` 记录来源和许可。若某一名称/配色的授权不适合分发，则保留相同用户覆盖面，以原创配色和非上游商标名称替换，并在本计划 `Decision Log` 记录偏差，不能静默删除主题数量或退回单色版本。
- 主题预览始终来自 registry 的实际 primary/secondary/tertiary/surface 值，不使用与应用 token 无关的营销颜色。

### 3. Preferences 与缓存兼容迁移

- Rust/TypeScript 的 canonical `Preferences` 改为 `themeId`，移除对外 `theme`/`palette` 组合。Rust preference 读取路径使用显式兼容 representation/decoder，接受旧 JSON 或新 JSON，返回 canonical `ThemeId`；保存只写新格式。SQLite 仍使用同一 key/value row，不升级 schema/user_version。
- 旧组合映射固定为：

| 旧 preference | 新 `ThemeId` |
| --- | --- |
| light + blue | `aurora-light` |
| light + emerald | `aurora-light`（保留青绿 secondary） |
| light + violet | `rose-pine-dawn` |
| light + amber | `gruvbox-light` |
| dark + blue | `aurora-dark` |
| dark + emerald | `nord` |
| dark + violet | `catppuccin-mocha` |
| dark + amber | `gruvbox-dark` |

- 缺失/未知/损坏 ID 安全回退 `aurora-light`；显式 `themeId` 优先于同一 JSON 中残留的 legacy 字段。读取 legacy 数据后不在 bootstrap 阶段额外写 SQLite，避免启动时无意义 mutation；用户下一次保存任一 preference 时自然写回 canonical JSON。
- localStorage 使用新 key `unlockm-theme-id`。`cachedThemeId()` 先读取有效新 key，再按上表迁移旧 `unlockm-theme`/`unlockm-palette`；成功应用后写新 key并删除两个旧 key。localStorage 不可用时回退默认，Rust bootstrap 仍是最终权威。
- `applyTheme(themeId)` 同步设置根节点 theme dataset 与 `style.colorScheme`，不 remount `App`、不重置 route、播放器、表单或滚动状态。bootstrap 和用户选择都复用同一函数。

### 4. 单一、可预览的主题选择器

- `AppHeader` 移除独立 palette 和 light/dark 控件，只保留一个带 Palette 图标的“外观/主题”按钮，避免经典 dark 主题又被 Moon/Sun 强行切回组合状态。
- picker 使用 viewport-bounded 两列 option grid；窄窗口改为单列并允许内部滚动。每个 `menuitemradio` 展示主题名、light/dark 小标签、四格色样/小型渐变和固定宽度选中 check slot，不能因选择状态改变文字位置。
- 沿用现有 menu 可访问性：打开后 focus 当前主题；ArrowLeft/Right 在同排移动，ArrowUp/Down 按列移动，Home/End 到首尾；Enter/Space 选择，Escape/外点关闭，关闭后恢复 trigger focus。触屏/鼠标 hover 只能预览 option 自身，不临时切换全局主题，避免背景快速闪烁。
- 选择后立即应用、写缓存并通过现有 narrow `setPreferences` 持久化；菜单关闭。持久化错误继续走全局 error banner，已应用的主题保持当前会话选择，下一次 bootstrap 由 Rust 权威值对齐。
- 三语言主题名称保留经典专名，说明文本和 light/dark/外观文案本地化；不得把色样作为唯一可识别信息。

### 5. 年轻化但克制的全局视觉语言

- `body`/`.app-shell` 使用主题提供的静态多层 radial/linear ambient gradient 与纯色 fallback。环境色保持低透明度、避开正文高密度区域，不使用随机或随时间变化的背景。
- 顶栏使用主题化 translucent/tinted surface、细边界和 backdrop blur；品牌 icon 获得轻量 colored shadow。图标按钮在 hover/active/focus 时使用对应主题的 secondary/selected surface，而不是全部变成 primary。
- composer 是首页视觉重点：drop zone 使用 tinted card、环境边缘光和更明显的 dragging/selected 状态；glyph、文件/文件夹按钮、output card、settings row 分配 primary/secondary/tertiary 层次；Convert CTA 使用主题双/三色 action gradient，hover 有轻微亮度/位移反馈，disabled 仍满足可辨识度。
- 历史区保持表格式扫描效率，但 header、active run、hover row、result/status 和 load-more 通过主题 surface/状态 token 增强层次；不能把每行做成互相竞争的大面积彩色卡片。
- 详情页的 back pill、summary card、filters、song header/row、active track、pagination 和 metadata links 使用统一的 selected/active/focus 语义。成功/失败/冲突仍有图标/文本，不只依赖颜色。
- 元数据 modal/backdrop、local artwork 和 online provider section 使用主题化 overlay、tinted panel 与 action hierarchy；保持现有 focus trap、内部滚动、固定 actions 和 provider 状态机。
- 固定播放器使用 tinted/glass floating surface、主题化进度/音量 accent、活动封面 halo 和清晰的 collapsed/expanded control；不改变固定位置、收展几何、音量/循环语义或 artwork 生命周期。
- 所有 component CSS 中现有固定 hex/rgb、黑色 shadow/backdrop 和未定义 `--line` 都迁移至主题 token；除 `themes.css`、必要的 transparent/currentColor 和图片本身外，不保留主题相关颜色字面量。

### 6. Motion、主题切换与辅助功能

- 微交互沿用 `--motion-fast`、`--motion-standard`、`--ease-standard`/`--ease-emphasized`：hover/press 以颜色、shadow 和最多 1–2px 的位移为主，时长 160–260ms，不触发大面积 layout shift。
- 主题切换可短暂过渡 canvas/surface/border/color/shadow，但不得给所有 DOM 建立通用 `transition: all`；图片、歌词/列表位置和固定播放器几何不参与主题动画。
- `prefers-reduced-motion: reduce` 下关闭位移、渐变位置变化和装饰 transition；`forced-colors: active` 下保留系统可识别的边界、focus 与选中态。
- 自动对比检查至少保证普通文本 4.5:1、大文本/图标/控件边界和 focus 3:1；primary CTA、selected row、status surfaces、menu option 和 modal 在全部主题中纳入检查。渐变上的文字按最弱 stop 验证，不能只测平均色。
- focus ring、选中 check、dragging、成功/警告/失败和 active track 都必须同时有结构/图标/文字或边界信号，不以色相作为唯一提示。

### 7. 扩展点、文档与并行计划边界

- 新增主题只允许改 registry、完整 theme token block、i18n/attribution 和相应测试；组件不得新增 `data-theme="..."` 特判。该规则和主题 token 边界完成后写入 `ARCHITECTURE.md`。
- `README.md`/`README.zh.md` 只说明内置主题、默认值、显式选择和旧 preference 自动迁移；不承诺系统跟随、自定义主题或在线主题下载。
- 实施开始时先复查两个 `In Review` 计划和工作树。若在线 metadata 或既有 UX plan 在同一 CSS/组件上出现新改动，以合并后的行为为基线，并在本计划 `Discoveries` 记录；不得回滚它们的未提交/新提交工作。
- 本计划不要求修改 `ARCHITECTURE.md` 中转换核心边界，只补充 desktop theme registry、prepaint、preference compatibility 和 CSS extension rule。

## Milestones

### M1 — Theme contract, catalog, and compatible persistence

- Goal: 建立完整主题 registry/token 契约和 canonical `ThemeId`，旧 Rust/localStorage preference 无损启动并可自然迁移。
- Dependencies: 当前两个 `In Review` 计划的生产代码作为只读基线；无需关闭其外部 QA 缺口。
- Acceptance criteria:
  - 9 个稳定 ID 均有 registry metadata、scheme、真实多色 preview 和完整 token block；无 selector 依赖 palette accent overlay。
  - Rust/TypeScript bootstrap、setPreferences、fixture 和缓存全部使用 `themeId`；旧 8 组合按既定映射读取，未知值安全 fallback。
  - cached prepaint 在 React mount 前生效，Rust bootstrap 可校正缓存；切换不丢失播放器/route/form state。
  - 经典主题来源和许可被核对；需要的 notice 已记录，无法兼容的上游有按本计划规则记录的原创替代。
- Validation:
  - `rtk vp -C desktop test --run theme.test.ts preferences.test.ts contracts.test.ts`
  - `rtk cargo test --locked -p unlockm-desktop preferences -- --test-threads=1`
  - `rtk cargo test --locked -p unlockm-desktop --test license_audit`
  - 手工冷启动 legacy localStorage + legacy SQLite fixture，确认无浅色闪烁、映射正确、下一次保存写 canonical JSON。

### M2 — Unified appearance picker

- Goal: 用一个 IDE 风格的完整主题选择器替代 palette 菜单与 light/dark toggle。
- Dependencies: M1 registry、apply/cache 和 preference contract。
- Acceptance criteria:
  - header 只有一个外观入口；9 个 option 显示名称、scheme、4 色 preview 和稳定 check slot。
  - 单/双列键盘导航、Enter/Space、Home/End、Escape、外点、焦点恢复和 viewport scrolling 均可用；320px 宽度不溢出。
  - 选择立即应用并持久化，重新启动保持；中/英/日 key parity 通过。
  - App interaction tests 不再断言组合式 palette 或 Moon/Sun toggle。
- Validation:
  - `rtk vp -C desktop test --run App.test.tsx locales.test.ts`
  - `rtk vp -C desktop check --no-fmt`
  - 手工仅键盘遍历全部主题、窄窗口触屏/滚动、持久化失败 error banner 和重启恢复。

### M3 — Coordinated youthful visual refresh

- Goal: 将完整 token 应用于所有主要界面，使配色、渐变和交互明显更丰富，同时维持内容效率和一致性。
- Dependencies: M1 token；M2 picker 可在实施过程中用来逐主题检查。
- Acceptance criteria:
  - canvas/header/composer/history/detail/player/modal 使用统一 ambient、surface、brand 和 state 角色；组件 CSS 无遗留 palette selector、主题色字面量或未定义变量。
  - 首页 CTA、drag state、active run/track、selected filter/menu、播放器和 metadata actions 在全部主题中具有清晰但不冲突的视觉层级。
  - 920px/820px/720px/680px 断点、sticky back、scroll-top、error banner、fixed player、modal viewport 和歌曲表格列无回归。
  - reduced-motion/forced-colors 有可用 fallback；主题切换不造成 route、播放、表单和滚动状态重置。
- Validation:
  - `rtk vp -C desktop test`
  - `rtk vp -C desktop build`
  - `rtk rg -n '#[0-9a-fA-F]{3,8}|rgb\(' desktop/src/App.css desktop/src/styles/history.css desktop/src/styles/player.css desktop/src/index.css`，结果只能是审查后明确允许的非主题例外，否则迁移到 token。
  - macOS debug app 在 920×640、1080×760、1280×800 检查 empty/selected/dragging/busy home、history/detail、active player、metadata modal/online section 和 error banner。

### M4 — Theme quality gates, documentation, and native review

- Goal: 用自动契约/对比测试、完整构建和双平台视觉矩阵把主题系统收口为可维护扩展点。
- Dependencies: M1–M3。
- Acceptance criteria:
  - 自动测试对 9 个主题执行 required-token completeness、ID uniqueness、scheme、preview-token 一致性和关键前景/背景对比检查。
  - `ARCHITECTURE.md`、README、notices 和本计划反映最终 theme/persistence/migration/extension 语义。
  - macOS 与 Windows 原生 WebView 均验证 picker/persistence、文字渲染、透明/blur/gradient、focus、reduced motion 和关键布局；无平台证据时计划保持 `In Review` 并明确缺口。
  - 全部 Vite+、desktop Rust、Tauri no-bundle 和 diff gates 通过，无未处置 review finding。
- Validation:
  - `rtk vp -C desktop fmt . --check`
  - `rtk vp -C desktop check --no-fmt`
  - `rtk vp -C desktop test`
  - `rtk vp -C desktop build`
  - `rtk cargo fmt --all -- --check`
  - `rtk cargo clippy --locked -p unlockm-desktop --all-targets -- -D warnings`
  - `rtk cargo test --locked -p unlockm-desktop --all-targets`
  - `rtk vp -C desktop exec tauri build --debug --no-bundle`
  - `rtk git diff --check`

## Task Breakdown

- [x] T1.1 在 `desktop/src/lib/theme-presets.ts` 定义 `ThemeId`、9 个 preset metadata、scheme、preview 和稳定查询 helper；为 ID uniqueness/default/preview 完整性添加 `theme-presets.test.ts`。
- [x] T1.2 将 `desktop/src/styles/tokens.css` 拆分为结构 token 与 `desktop/src/styles/themes.css` 的完整主题 color blocks，移除 `data-palette` 分支；增加 required-token completeness 和颜色字面量边界测试。
- [x] T1.3 在 `desktop/src-tauri/src/history/mod.rs` 将 canonical preference 改为 `theme_id`，增加旧 `theme`/`palette` JSON 的兼容 decoder、8 组合映射、未知值 fallback、canonical serialization 与 idempotent reopen tests。
- [x] T1.4 更新 `desktop/src/lib/contracts.ts`、`preferences.ts`、`ipc.ts` 相关类型/fixture/tests，使 bootstrap 和 setPreferences 只暴露 `themeId`，并验证 locale/default settings/player volume 不受迁移影响。
- [x] T1.5 重构 `desktop/src/lib/theme.ts` 与 `main.tsx`：实现新/旧 localStorage 迁移、同步 apply、scheme 设置、旧 key 清理和 pre-mount paint；覆盖 unavailable storage、unknown ID、legacy precedence 和无状态重置回归。
- [x] T1.6 核对 7 个经典主题的官方 palette 参考、版本/commit 和许可证；按结果更新 `THIRD_PARTY_NOTICES.md` 与 `desktop/src-tauri/tests/license_audit.rs`，或按既定 fallback 记录原创替代决定。

- [x] T2.1 在 `desktop/src/App.tsx` 将 theme/palette state、bootstrap、persist callback、locale/volume/settings 保存和 header props 收敛为单一 `themeId`，覆盖选择、bootstrap 校正和 preference snapshot tests。
- [x] T2.2 重构 `desktop/src/components/AppHeader.tsx`，用 registry 驱动的单/双列 appearance picker 替换两个旧控件，完成 fixed check slot、scheme badge、4 色 preview、二维 roving focus、viewport bound 和 focus restore。
- [x] T2.3 更新 `desktop/src/App.css` 的 picker/header 样式和 680px/320px 响应式行为，确保菜单不会被窗口裁切、不会遮挡品牌和不会因 option 选中移动文字。
- [x] T2.4 更新 `desktop/src/i18n/locales/{zh-CN,en-US,ja-JP}.json` 与 parity tests，覆盖外观入口、主题名称/说明、light/dark 标签和选择反馈；经典专名不做误导性直译。
- [x] T2.5 重写 `desktop/src/App.test.tsx` 的旧组合主题用例，覆盖菜单默认关闭、9 options、二维键盘、Escape/外点、立即 apply、持久化 payload、重启恢复和持久化失败提示。

- [x] T3.1 在 `desktop/src/index.css`、`tokens.css`、`themes.css` 和 `App.css` 建立 app canvas ambient layers、theme transition boundary、tinted header、brand/icon controls，并实现 reduced-motion/forced-colors fallback。
- [x] T3.2 在 `desktop/src/App.css` 与 `desktop/src/features/conversion/Composer.tsx` 的既有 class 边界内刷新 drop field、glyph、choice/output/settings、Convert CTA、busy/live run、history row/empty/load-more/error/skeleton 状态；不改变转换事件或组件 props。
- [x] T3.3 在 `desktop/src/styles/history.css` 刷新 detail header/back、facts、filters、song header/rows、active track、outcome、pagination 和 metadata links，保持现有 resize handle、点击区域、虚拟/分页结果和 820px layout。
- [x] T3.4 在 `desktop/src/styles/history.css` 和 `MetadataEditor.tsx` 既有结构内统一 modal backdrop/surface、local cover、online provider/result/actions 的主题层级，移除 `--line` 和硬编码 overlay，保持 focus trap、scroll、draft/Save 边界。
- [x] T3.5 在 `desktop/src/styles/player.css` 刷新 fixed player surface、cover halo、progress/volume、play/collapse/error states，验证 expanded/collapsed 几何、safe-area、scroll-top/error banner 避让与 artwork object URL 行为不变。
- [x] T3.6 对 `App.css`、`history.css`、`player.css` 和 `index.css` 做主题色字面量/undefined variable/`transition: all` 审计；将合法视觉值收口为 token，并添加或更新对应组件 regression tests。

- [x] T4.1 新增数据驱动的 theme contract/contrast tests，对全部 preset 验证 required tokens、scheme、preview 来源、正文/次级文字、CTA、focus、selected、status 和 modal 的阈值；对渐变按最弱 stop 验证。
- [x] T4.2 扩展 `App.test.tsx`、`MetadataEditor.test.tsx`、`PersistentPlayer.test.tsx` 与相关 interaction tests，覆盖换主题时 route/播放/表单/滚动不重置、reduced-motion class/媒体查询契约和 status 不只依赖颜色。
- [x] T4.3 更新 `ARCHITECTURE.md`、`README.md`、`README.zh.md`，记录 theme registry/token 扩展点、cached prepaint/Rust authority、legacy preference 映射和内置主题范围。
- [x] T4.4 执行 M4 完整自动 validation，逐条把命令、结果、警告和平台写入本计划 `Progress`/`Completion Evidence`；失败、skip 和既有警告都不能记为 pass。
- [ ] T4.5 在 macOS 和 Windows 原生 app 完成视觉矩阵：全部 9 主题检查 picker/关键 surface/对比；Aurora Light、Tokyo Night、Gruvbox Light 额外覆盖 920×640/1080×760/1280×800、empty/selected/dragging/busy/detail/player/modal/error、键盘、reduced motion 和重启持久化，并保存可审查截图/记录。
- [ ] T4.6 完成独立 review，逐项关闭主题完整性、迁移、对比、响应式、固定层遮挡、许可和文档 findings；仅在所有 in-scope task 与双平台证据完成后转为 `Done`。

## Validation Strategy

### Automated frontend

- Theme registry/token：ID 唯一、default 存在、9 个 selector/required token 完整、registry preview 与实际 token 同源或显式一致、无 component-level theme special case。
- Theme application/cache：新 key、8 个 legacy mapping、unknown/corrupt/unavailable storage fallback、dataset、`color-scheme`、旧 key 清理、Rust bootstrap authority。
- Preferences/IPC：canonical `themeId` fixture、locale/settings/volume snapshot、setPreferences payload、旧 JSON decoder 与 canonical reserialization。
- Accessibility：关键 foreground/background 对比阈值、focus token、selected/status 的非颜色标记、picker keyboard/focus lifecycle、reduced-motion/forced-colors CSS contract。
- Component regression：主题切换不 remount route/player/editor；header menu、metadata modal、persistent player、home/detail navigation 和现有转换交互保持通过。

### Automated Rust and build

- Rust history tests覆盖 missing theme、legacy light/dark + 4 palette、new ThemeId、unknown value、read/reopen、set/get 和 camelCase bootstrap fixture。
- license audit 验证随发行包加入的 theme notice/source；若只有 notice 而无独立 asset，测试至少锁定 notice 中的 required entries。
- 运行 Vite+ fmt/check/test/build、desktop Rust fmt/clippy/test、Tauri debug no-bundle 和 `git diff --check`。主题不触及 root core，但若实施改动超出 `desktop/`，追加受影响 crate 的完整 locked test matrix。

### Manual visual QA

- 首先在 1080×760 对全部 9 主题检查 header picker、canvas、composer、history row、detail summary/song row、active player、modal 与 status colors，确认每套都体现多色层次而非只替换 primary。
- 再用 Aurora Light、Tokyo Night、Gruvbox Light 代表明亮原创、深色经典、暖色浅色，在 920×640、1080×760、1280×800 覆盖主要 UI 状态和中/英/日长文案。
- 在 macOS WKWebView 与 Windows WebView2 检查 gradient、alpha、backdrop-filter、shadow、native form/range `color-scheme`、高 DPI/跨显示器、键盘 focus、reduced motion 和重启持久化。
- 检查主题切换时正在播放的歌曲、volume、detail route、metadata draft、filter、column split 和 scroll position保持不变；fixed player、scroll-top、error banner、picker 和 modal 不互相遮挡。
- 对 contrast 自动测试无法覆盖的透明叠层、blur、封面颜色干扰和 disabled 状态进行人工审查。若 Windows 不可用或经典主题授权未核清，计划保持 `In Review` 并在 Completion Evidence 明确缺口。

## Risks and Mitigations

- **完整主题数量导致 QA 组合爆炸。** 自动对所有 9 套执行 token/contrast 契约；全部主题只做标准尺寸关键面，窗口/状态全矩阵集中在 3 套代表主题，兼顾覆盖与可执行性。
- **旧 preferences 直接改字段会导致 bootstrap 失败。** Rust 使用显式兼容 decoder，前端缓存使用确定映射，fixture 覆盖 8 个组合；不依赖 serde unknown-enum 自动 fallback。
- **缓存预绘制与 Rust preference 不一致造成闪烁。** React mount 前同步应用 cache，bootstrap 后只在权威值不同才对齐；default CSS 与 Aurora Light 一致，并在原生冷启动检查深色主题。
- **渐变/透明层在不同主题或封面背景下损害对比。** 正文落在稳定 opaque/tinted surface，gradient 主要用于 canvas、装饰和 CTA；对渐变最弱 stop 做对比测试并在原生 WebView复核。
- **经典主题名称、palette 和许可证存在来源风险。** 实施前锁定官方来源/版本/许可并写 notice；不导入代码。无法兼容时按相同 light/dark/色彩覆盖提供已记录的原创替代，不阻断主题系统架构。
- **与两个 In Review 计划的 `MetadataEditor.tsx`/CSS 重叠。** 实施开始先复查 git/计划，围绕最新结构只改视觉和 theme contract；不回滚 online provider、播放器或已有 UX 修复。
- **组件 CSS 继续出现硬编码颜色会让部分主题失真。** 增加颜色字面量与 undefined token 审计，主题相关字面量只允许存在于 `themes.css`/registry preview，并在 review 中作为阻断项。
- **通用颜色 transition 影响性能或 reduced motion。** 只给顶层/交互组件列出具体 property，禁止 `transition: all`；reduced-motion 关闭位移和装饰 transition。
- **Windows WebView2 对 blur、range、alpha 的表现不同。** 保留不依赖 blur 的 opaque fallback，使用 native `color-scheme`，并把 Windows 原生检查作为 Done 前必须证据而非 CI 编译替代。

## Progress

- [x] 2026-08-25：读取用户指定的 `plan-doc`、`AGENTS.md`、父级 `RTK.md`、`docs/PLANS.md`、`ARCHITECTURE.md`、CI、package/Vite+ 配置和现有 plans；确认没有覆盖本目标的 active plan。
- [x] 2026-08-25：检查 theme/cache/preferences/bootstrap/header、App/composer、history/detail/player/metadata styles、Rust preferences JSON 与相关 tests；确认当前 palette 仅覆盖 accent，且新计划与两个 `In Review` 计划的边界可分离。
- [x] 2026-08-25：创建本 `Ready` ExecPlan；只修改规划文档，未改生产代码、测试、配置或依赖。
- [x] 2026-08-25：启动 implementation；采用 `DESIGN_VARIANCE 7 / MOTION_INTENSITY 5 / VISUAL_DENSITY 7`，保留信息架构与现有功能语义，按 M1-M4 顺序实施并进入 review/fix/re-review 循环。
- [x] 2026-08-25：M1 完成。新增 9-theme registry、完整 CSS token blocks、Rust/TypeScript canonical `themeId`、8 组合兼容 decoder、localStorage migration、fixture/tests 与 pinned theme notices；进入 M2。
- [x] 2026-08-25：首次 `rtk vp -C desktop test --run ...` 因 worktree 缺少 `desktop/node_modules` 启动失败；执行 `rtk vp -C desktop install --frozen-lockfile` 成功恢复 151 个锁定包，未修改 lockfile，随后同一测试通过。
- [x] 2026-08-25：theme contract test 的首版 `?raw`/`import.meta.url` 读取方式在 Vite+ runner 下不可用；改为从 `process.cwd()` 读取受版本控制的 CSS 后，5 files / 18 tests 通过。这是 test harness 兼容修复，不改变生产主题加载。
- [x] 2026-08-26：M2 完成。App preferences、bootstrap 和 persist 全部收敛为 `themeId`；header 合并为单一 9-theme picker，提供 preview/scheme/check、二维 roving focus、Escape/外点和 520px 单列 fallback；三语言 parity 与 App tests 通过。
- [x] 2026-08-26：`rtk vp -C desktop check --no-fmt` 为 0 error / 9 warnings。warnings 均位于既有 App boot effect、HistoryDetail/Playback state effects 和 MetadataEditor `Date.now()` purity，不由 theme diff 新增；在 review 阶段重新评估，不误记为全绿 lint。
- [x] 2026-08-26：M3 完成。全局 canvas、header、composer、history/detail、song list、fixed player、metadata modal/online section、error/status 均接入多色 ambient/surface/gradient/state 体系；fixed color、undefined semantic token、`transition: all` 和 component theme selector 由 contract test 锁定。
- [x] 2026-08-26：完整 `rtk vp -C desktop test` passed (19 files, 112 tests)，`rtk vp -C desktop build` passed；仅保留既有 ineffective dynamic import 与 >500 kB chunk warnings。后续 contract/App/MetadataEditor/PersistentPlayer 定向复验 passed (4 files, 45 tests)，`rtk git diff --check` passed。
- [x] 2026-08-26：`rtk vp -C desktop exec tauri build --debug --bundles app` 成功生成 `target/debug/bundle/macos/UnlockM.app`。Computer Use 原生 QA 覆盖 Rosé Pine Dawn home、Tokyo Night home/detail/player/modal、Gruvbox Light home/detail/player、theme restart persistence 与 1080×760/920×640；播放测试后立即暂停，未修改音乐或 metadata。
- [x] 2026-08-26：M3 记录当前 Mac 不能提供精确 1280×800；后续已补 1365×768 full-screen，但 exact height 与 Windows 继续由 T4.5 唯一追踪，不计为 pass。
- [x] 2026-08-26：M4 implementation、四轮 review/fix 和 macOS 可执行矩阵完成；只剩 T4.5 外部平台/尺寸证据。
- [x] 2026-08-26：T4.1-T4.4 完成。第一次 contrast gate 暴露 19 个 text/state/border/gradient stop 对比问题；逐主题修正 token 后 `theme-contract.test.ts` 4 tests 通过，未降低 4.5:1/3:1 阈值。
- [x] 2026-08-26：增加 theme switch 不重置 detail route/scroll、persistent player/collapsed state 和 metadata unsaved draft 回归；移除设计预检禁止的 scroll event listener，改用 Motion `useScroll/useMotionValueEvent`；可见 fallback em dash 改为普通 hyphen。
- [x] 2026-08-26：M4 自动矩阵完成。最初 format check 因最新 `App.tsx` import 行失败，执行 Vite+ formatter 后复验通过；full frontend 19 files / 117 tests、build、Rust fmt、desktop clippy、desktop 89 tests、Tauri no-bundle、diff check 均通过。Vite+ check 为 0 errors / 9 React Compiler warnings，交由 review 判定。
- [x] 2026-08-26：进入第一次 `review-plan` 时保留 T4.5 外部 gap；之后共完成四轮 review/fix，最终 code/plan review 为 Pass with Notes。
- [x] 2026-08-26：第一次 review 为 `Needs Changes`，记录 1 Major / 2 Minor 与 T4.5 validation gap；切回 implementation 逐项修复后 re-review。
- [x] 2026-08-26：review remediation ready。App boot 延迟到可取消 timer；HistoryDetail 以 request key 派生 loading/error 并让 artwork/visibility 不再同步 effect setState；Playback 以 prop-source state 派生 volume、封面以 artwork ID 匹配 loaded URL；MetadataEditor 用可清除 cooldown timer 取代 render-time `Date.now()`；Vite+ check 已 clean。
- [x] 2026-08-26：修复 9 项两列 picker 的同列首尾 wrap，新增左右列 ArrowUp/Down 回归；新增 theme persistence rejection、optimistic session、error banner 和下一次 bootstrap authority 回归。定向 `check` clean，6 files / 80 tests passed；进入第二次 review。
- [x] 2026-08-26：第二次 review 验证第一次 findings 全部关闭，但发现 rapid A→B→A artwork revoked-URL race 和主题扩展文档遗漏 Rust enum/test；切回 implementation。
- [x] 2026-08-26：第二轮 remediation ready。PlayerArtwork 按 artwork/track key、History Artwork 按 artwork/visibility key remount，新增两条 A→B→A deferred return regressions，确保不会渲染已 revoke URL；Architecture theme extension 已包含 Rust `ThemeId` 与 serialization/migration tests。Vite+ check clean，3 files / 42 tests passed；进入第三次 review。
- [x] 2026-08-26：第三次 code re-review initially clean（frontend 19 files / 120 tests），但随后 T4.5 全 9 主题原生矩阵发现 WKWebView 对 `transition: background` + custom-property theme switch 保留旧 surface/gradient；Aurora Dark 出现 dark canvas + light controls 的混合状态。该 native finding 重新打开 review 并切回 implementation。
- [x] 2026-08-26：WKWebView theme-switch remediation ready。component transitions 收窄为 `background-color`，`applyTheme` 以临时 `.is-theme-switching` 禁用 transitions、写入 dataset/scheme、强制 style flush 后恢复普通 hover motion；contract 禁止 shorthand background transition。重建 absolute bundle 后全 9 主题 1080×760 重新检查，Aurora Light→Dark 与 Dark→Light 首帧均完整一致；进入第四次 review。
- [x] 2026-08-26：第四次 review 为 `Pass with Notes`，无剩余 code/test/migration/accessibility/plan finding。最终 clean matrix：Vite+ fmt、check（43 files clean）、19 files / 120 tests、build、Rust fmt、desktop clippy、desktop 89 tests、Tauri no-bundle 与 diff check 全部通过；build 仍只有既有 dynamic-import/chunk-size warnings。
- [x] 2026-08-26：T4.5 macOS 可执行范围完成：重建 absolute bundle 后全部 9 主题在 1080×760 逐一检查，Aurora Light/Dark 原子双向切换、Tokyo detail/player/modal、Gruvbox Light、Rosé Pine Dawn、920×640 和 1365×768 full-screen 均有原生证据。当前物理屏幕最大 1365×768，无法提供精确 1280×800 高度。
- [x] 2026-08-26：将主题提交集成到 `main` 的 `5123c6c`（新版 split-workspace metadata editor）之上；冲突解决保留新版编辑/联网结果布局，同时移植 cooldown timer/isRateLimited 纯渲染修复，并让新增 input/select、候选卡与在线 pane 使用完整主题 semantic tokens。合并后 Vite+ check clean、19 files / 124 tests、build、desktop clippy、desktop 89 tests 与 macOS app bundle 均通过；Rosé Pine Dawn/Tokyo Night 原生 modal 复查无布局、滚动、对比或主题切换回归。
- [ ] T4.5 Windows 外部 gap：Computer Use app inventory 未发现 Parallels、UTM、VMware、VirtualBox、Wine、Whisky 或 CrossOver；当前 Mac 无可信 Windows WebView2 runtime。Windows theme/picker/blur/range/DPI/跨显示器不能由 macOS 编译或截图推断，仍阻止计划转 `Done`。

## Discoveries

- 当前“8 种主题组合”实际只有两套完整 surface/text token；四种 palette 只覆盖 accent/focus/border accent，这正是用户感知主题单一的直接原因。
- 现有组件已经较一致地消费 CSS variables，因此不需要换框架或重写组件；主要工作是扩展语义 token、清理局部字面量并按界面层级重新分配角色。
- 顶栏 palette menu 已具备大部分正确键盘/焦点模式，适合升级为 theme picker；继续保留独立 Moon/Sun 会与完整 classic preset 的内置 scheme 冲突。
- Preferences 存储是单条 JSON，不需要 SQLite schema migration；真正风险是 serde/TypeScript/localStorage 三处兼容读取和 bootstrap 权威顺序。
- 当前 component CSS 除 palette swatch 外仍有固定黑色 shadow/backdrop，在线 metadata border 使用未定义 `--line`；完整主题需要将透明层、shadow 和 overlay 也纳入 token，不能只增加更多 accent selector。
- 现有测试栈没有截图基线，但 registry 可让 required token、scheme、preview 和 contrast 做数据驱动测试；视觉层次、blur 和跨平台 alpha 仍必须依赖原生 QA。
- 当前 Motion/reduced-motion 基础已经足够，年轻化不需要增加持续动画或新依赖；颜色、surface、轻量位移和清晰 selected/drag states 即可形成明显变化。
- 官方 palette 核对显示 Tokyo Night 上游为 Apache-2.0；Catppuccin、Dracula、Nord、Rosé Pine 为 MIT，Gruvbox 官方 README 声明 MIT/X11。主题 notice 必须逐项记录，不能把所有经典主题概括为 MIT。

## Decision Log

- **2026-08-25 — D1:** 新建独立增量计划，不修改 `2026-08-23-desktop-history-player-metadata-ux.md` 或 `2026-08-25-online-metadata-search-spotify.md` 的状态；它们的当前实现是本计划基线。
- **2026-08-25 — D2:** 用单一完整 `ThemeId` 取代 `theme × palette`，不继续增加 accent-only palette，也不保留独立 light/dark toggle。
- **2026-08-25 — D3:** 首批固定为 9 套完整预设，`UnlockM Aurora Light` 是 fresh-install default；Tokyo Night 等经典主题必须定义全套 surface/content/state/gradient tokens。
- **2026-08-25 — D4:** 旧 8 组合按计划表确定性映射；Rust 兼容 decoder 和 localStorage migration 同时保留，SQLite schema 不变。
- **2026-08-25 — D5:** 年轻化方向采用低透明 ambient canvas、分层 tinted surfaces、多色 CTA/selected/drag 状态和克制微交互；不引入图片、持续背景动画或信息架构重写。
- **2026-08-25 — D6:** 主题 registry + 完整 token block 是唯一扩展点，组件不得按主题 ID 特判；component CSS 的主题色字面量和未定义变量作为 review blocker。
- **2026-08-25 — D7:** 自动对全部主题做完整性/对比检查，原生全状态矩阵集中在 3 套代表主题；Windows 原生证据缺失时不能标记 Done。
- **2026-08-25 — D8:** 经典 palette 固定到 notice 中记录的 6 个官方上游和具体 commit；只转译颜色值，不复制 editor integration、代码或图片资产。Tokyo Night 保留 Apache-2.0 的单独声明。
- **2026-08-26 — D9:** 当前 Mac 的 large native evidence 使用可用最大 1365×768，补充而不冒充 1280×800；920×640/1080×760 仍为精确窗口证据。Windows 无本地/虚拟化 runtime，因此 T4.5 保持 external validation gap，不用 CI 编译或 macOS 视觉结果替代。

## Completion Evidence

- M1 frontend: `rtk vp -C desktop test --run theme-presets.test.ts theme-contract.test.ts theme.test.ts preferences.test.ts contracts.test.ts` passed (5 files, 18 tests).
- M1 Rust: `rtk cargo test --locked -p unlockm-desktop preferences -- --test-threads=1` passed (2 matching tests); `rtk cargo test --locked -p unlockm-desktop typescript_bootstrap_fixture_matches_rust_camel_case_contract -- --test-threads=1` passed; `rtk cargo test --locked -p unlockm-desktop --test license_audit` passed.
- M2 frontend: `rtk vp -C desktop test --run App.test.tsx locales.test.ts theme-presets.test.ts theme-contract.test.ts theme.test.ts preferences.test.ts contracts.test.ts` passed (7 files, 31 tests); `rtk vp -C desktop check --no-fmt` completed with 0 errors and 9 recorded pre-existing React Compiler warnings.
- M3 frontend/build: full Vite+ test passed (19 files, 112 tests); build passed with two recorded non-blocking chunk/import warnings; focused visual-contract/component suite passed (4 files, 45 tests); component color-literal scan returned no matches and semantic-variable audit found only the two intentional HistoryDetail runtime track variables.
- M3 native macOS: debug `.app` build passed; 1080×760 and minimum 920×640 WKWebView screenshots/checks covered home/detail/player/modal across representative light/dark presets, keyboard-accessible picker state and restart persistence. 1280×800 and Windows remain open M4 evidence.
- M4 automated: final `rtk vp -C desktop fmt . --check` passed; `check --no-fmt` completed with 0 errors/9 recorded warnings; full `test` passed (19 files, 117 tests); `build` passed with the recorded dynamic-import/chunk warnings; `rtk cargo fmt --all -- --check`, desktop clippy `-D warnings`, desktop tests (89), Tauri debug no-bundle, and `rtk git diff --check` passed.
- M4 final after review remediation: `rtk vp -C desktop fmt . --check` passed; `rtk vp -C desktop check --no-fmt` passed clean (43 files, 0 warnings/errors); full `rtk vp -C desktop test` passed (19 files, 120 tests); build passed with the two recorded pre-existing warnings; Rust fmt, desktop clippy `-D warnings`, desktop tests (89), Tauri debug no-bundle, and diff check passed.
- M4 final macOS native: absolute debug bundle rebuilt after each native finding. All 9 themes passed the 1080×760 home catalog; representative detail/player/modal and 920×640 passed; 1365×768 full-screen covered the available large width. Atomic Light→Dark and Dark→Light screenshots confirm no stale transitioned surface/text. Windows and exact 1280×800 remain explicit gaps.
- Main integration: cherry-pick onto `5123c6c` resolved the redesigned `MetadataEditor.tsx`/`history.css` at behavior and semantic-token level. `rtk vp -C desktop check --no-fmt` passed clean; `test` passed 19 files / 124 tests; `build` passed with only the recorded dynamic-import/chunk warnings; desktop clippy/tests passed (89); `tauri build --debug --bundles app` passed. Native Rosé Pine Dawn and Tokyo Night checks covered the merged split editor, independent pane scrolling, footer actions, form controls and immediate theme switching.
- 完成时仍须记录：最终 commit(s)、完整 Vite+/Rust/Tauri 命令结果、macOS/Windows 原生截图或检查记录、已处置警告/偏差和独立 review 结论。

## Review Notes

- [x] **Major, resolved on second review:** Vite+ check 已从 9 warnings 变为 43 files clean；App/HistoryDetail/Playback/MetadataEditor 的 effect/purity remediation 保持了 query race、volume、cooldown 和 route/player/editor state tests，完整 frontend 19 files / 118 tests 通过。
- [x] **Minor, resolved on second review:** 两列奇数网格现在按 column 求最后一项，左右列首尾 ArrowUp/Down 均有 App 回归。
- [x] **Minor, resolved on second review:** `setPreferences` rejection 测试覆盖 optimistic theme、menu close、error banner/detail 和新 App bootstrap 的 Rust authority correction。
- [x] **Major, resolved on third review:** artwork loader 按 ID/visibility keyed remount；History/Player 两条 deferred A→B→A regressions 证明返回旧 ID 时先显示 placeholder、只接受新 URL，并保持 revoke/late-response safety。
- [x] **Minor, resolved on third review:** Architecture extension point 已要求同时更新 Rust `ThemeId`、frontend registry、完整 CSS token、i18n/notice、Rust serialization/migration 和 frontend contract/contrast tests。
- [x] **Major, resolved on fourth review:** WKWebView theme switch 以临时 transition suppression + style flush 原子提交，component contract 禁止 shorthand background transition。重建 absolute bundle 后全 9 主题重跑，Aurora Light→Dark 与 Dark→Light 首帧均无旧 surface/text 残留。
- [x] **Review:** 第四次 holistic code/plan review 为 `Pass with Notes`，无未解决 implementation finding。唯一 note 是下方 T4.5 external validation gap。
- [x] **Main integration review:** `5123c6c` 的新版元数据分栏编辑器、封面反馈与搜索结果结构全部保留；主题提交只补入 cooldown 纯渲染修复和 semantic surface/control tokens。合并后自动矩阵与 Rosé Pine Dawn/Tokyo Night 原生复查均无新增 finding。
- [ ] **Validation:** T4.5 的 1280×800 与 Windows WebView2 原生矩阵仍未执行。该缺口已在 Progress/Completion Evidence 保留，当前没有被误报为 pass；在获得环境或调整计划的明确产品决定前，状态不得转为 `Done`。
