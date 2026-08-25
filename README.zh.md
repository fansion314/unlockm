# unlockm

[English](README.md)

`unlockm` 可以将受支持的加密音乐下载文件转换为普通、可播放的音频。它直接解密原始
音频流，不做转码，保留源文件，并可将输入目录结构原样映射到单独的输出目录。

## 为什么选择 unlockm？

- **音质无损：** 只解密，不重新编码音频。
- **默认安全：** 永不删除源文件；只有完整输出才会被原子发布。
- **适合处理整个曲库：** 递归扫描目录，并默认复制封面、歌词等附属文件。
- **反馈清晰：** 交互式终端会展示整体进度和当前正在处理的文件。
- **加密实现自包含：** 程序使用的格式解密代码都在本仓库内；运行时不依赖外部
  加密服务，也不依赖 FFmpeg。

## UnlockM 桌面应用（macOS 与 Windows）

`UnlockM` 是同一套 Rust 引擎的原生桌面宿主。目前支持 macOS 和 Windows；命令行
`unlockm` 仍是跨平台方案，并继续支持 Linux 与 watch 模式。

首版桌面应用刻意保持精炼：

- 选择或拖入**一个**文件或目录，并可选择输出目录；
- 转换前设置元数据更新和目录附属文件复制；
- 观察一个活跃转换、随时安全取消，并在重启后继续查看本地历史与详情；
- 在固定标题栏选择 9 套完整浅色/深色主题，包括 Tokyo Night、Catppuccin Mocha、
  Dracula、Nord、Gruvbox 与 Rosé Pine Dawn，并切换简体中文、英文或日文；
- 搜索、筛选、排序并分页查看歌曲，以及封面、歌手、专辑、转换后格式和实际文件大小；
- 每次播放一首转换后的歌曲，支持暂停和拖动进度，也可在 Finder/资源管理器中显示输出，
  或使用系统默认播放器打开歌曲。

原生“关于”面板感谢加性饼干提供词元，说明应用为魔仙煲和文森特李73制作，并标注
`Copyright © Hamlet Fansion (Peilin Fan)`。

新用户默认使用 UnlockM 极光浅色。每套内置主题都会独立定义背景、表面、文字、状态色、
渐变和原生表单配色，不再只是替换一个强调色。升级时，原有的浅色/深色与
蓝色/翡翠绿/紫罗兰/暖橙组合会自动映射到最接近的完整主题。

桌面版 v1 不包含 watch 监听、播放列表/队列、波形、云同步、账户、遥测或手动密钥输入。
转换完全在进程内完成：应用直接链接本仓库的 Rust 库，**不会**启动 `unlockm` 命令行、
sidecar、shell 命令或转换守护进程。只有用户明确点击“显示/打开”时，Rust 才会用历史记录
中的 run/item ID 解析并复验目标，然后请求系统显示该输出或用默认播放器打开。

### 桌面预览安装包与本地数据

每个 `v*` 标签会自动发布一个**未签名预览版 prerelease**：一个通用 macOS DMG 和一个
Windows x86_64 NSIS 安装程序，并各自附带 SHA-256 文件。工作流不需要 GitHub environment、
证书、密码或签名密钥。GitHub-hosted Actions 分钟数和 artifact 存储仍会计入仓库所有者的
GitHub 套餐与配额；私有仓库超额使用可能产生费用。macOS 应用只有 Tauri 的 ad-hoc 签名，
**没有公证**；Windows 应用和安装程序刻意保持 Authenticode `NotSigned`。它们是安装包，
而不是可移植的命令行二进制文件。

首次打开预览版前，只从对应的 GitHub prerelease 下载，并校验附带的哈希：

```sh
# macOS
shasum -a 256 UnlockM-*-macos-universal-unsigned.dmg

# Windows PowerShell
Get-FileHash .\UnlockM-*-windows-x64-unsigned-setup.exe -Algorithm SHA256
```

将结果与对应的 `.sha256` 发布附件比较。macOS 若在校验后仍出现 Gatekeeper 提示，请在 Finder
中对该应用使用**按住 Control 点击 → 打开**，再确认打开。Windows 若 SmartScreen 将已校验的
预览版识别为未知应用，只在你信任该发布和匹配哈希时使用该应用的**更多信息 → 仍要运行**。
不要关闭 Gatekeeper、SmartScreen 或其他系统级保护。未来可另行提供已签名/已公证的发布渠道。

UnlockM 把本地 `history.sqlite` 存在 Tauri 的应用数据目录中（通常为 macOS 的
`~/Library/Application Support/world.fansionia.unlockm/`，Windows 的
`%APPDATA%/world.fansionia.unlockm/`）。从旧版错误标识升级后，如果新目录还没有数据库，
UnlockM 会使用 SQLite 在线备份 API 安全复制 `world.fansion.unlockm/history.sqlite`，并保留旧
数据库作为可恢复副本。它只保存本地转换记录、绝对源/输出路径、设置快照、
结果详情、捕获的元数据、转换后文件类型/实际大小及按 SHA-256 去重的专辑封面（每张最多
10 MiB）；不保存音频负载，不发送遥测，也不会删除记录中的音乐文件。内置播放器通过仅含
item ID 的范围请求按需读取已复验输出。应用内的清空/移除只删除数据库记录。

### 桌面开发

前端生命周期只能通过 Vite+ 入口执行。项目通过 `desktop/pnpm-lock.yaml` 锁定 pnpm
后端；不要直接运行 npm、pnpm、yarn 或 bun。

```sh
vp -C desktop install --frozen-lockfile
vp -C desktop check
vp -C desktop test
vp -C desktop build
vp -C desktop exec tauri dev
```

若只需本地原生编译、不生成安装包：

```sh
vp -C desktop exec tauri build --debug --no-bundle
```

预览安装包由标签工作流自动发布，且不使用任何发布密钥。它与未来可选的已签名/已公证发布
渠道有意保持区分。

应用安装包也会随本地捆绑的 Bricolage Grotesque 与 Manrope 字体附带
[第三方声明](THIRD_PARTY_NOTICES.md) 和完整的
[SIL Open Font License 1.1](third_party/licenses/SIL-OFL-1.1.txt)。

## 安装

需要 Rust 1.89 或更高版本。锁文件是发布约定的一部分。

```sh
cd unlockm
cargo install --path . --locked
unlockm --version
```

如果只想构建、不安装：

```sh
cargo build --release --locked
./target/release/unlockm --help
```

## 快速开始

转换音乐目录，并把原目录结构映射到新的输出目录：

```sh
unlockm ~/Music/Encrypted -o ~/Music/Unlocked
```

转换音频，并以保守策略补全标签和封面：

```sh
unlockm ~/Music/Encrypted -o ~/Music/Unlocked --update-metadata
```

只转换加密音频，不复制歌词、图片等普通文件：

```sh
unlockm ~/Music/Encrypted -o ~/Music/Unlocked --no-copy
```

使用 `unlockm --help` 查看全部选项，使用 `unlockm --supported-ext` 查看当前版本完整的
扩展名列表。

## 文件会输出到哪里

默认的**镜像**布局会保留子目录：

```text
Encrypted/Artist/Album/song.ncm
                    ↓
Unlocked/Artist/Album/song.flac
```

除非设置 `--no-copy`，普通附属文件也会被复制。目标已存在时默认跳过；只有显式设置
`--overwrite` 才会替换，而且仍要等新文件完整生成后才执行替换。如果输出目录位于输入
目录内部，扫描和监视模式都会忽略它。

使用 `--flat` 可将所有结果直接放进输出根目录：

```sh
unlockm ~/Music/Encrypted -o ~/Music/Unlocked --flat
```

如果两个输入会产生相同的扁平化文件名，两者都会被报告为冲突；unlockm 不会随意选择
其中一个覆盖另一个。

## 进度与命令行输出

交互式终端会显示两行实时状态：

```text
[========--------------] 118/326  36.2%  2.4s
Decrypting: Artist/Album/song.ncm (+1 active)
```

每完成一个文件，计数就会立即前进。第二行显示最近活跃文件的相对路径；
`(+1 active)` 表示还有一个文件正在同时处理。

默认值 `--progress auto` 只在交互式终端播放进度动画。使用 `--progress always` 可强制
显示，使用 `--progress never` 可关闭。重定向的人类可读输出不含终端控制字符，并以稳定
摘要结尾：

```text
unlockm: converted=80, copied=246, skipped=0, conflicted=0, failed=0
```

脚本可使用 `--json-logs`；它会关闭实时进度，并输出 JSON 诊断记录和最终 JSON 摘要。
进度与诊断信息都写入 stderr。

## 并发与性能

`--jobs` 表示最多同时处理多少个**文件**。默认值为两个文件任务；它既不表示整次运行
只能处理两个文件，也不保证程序只使用两个操作系统线程。

在具有代表性的 3.4 GiB APFS 混合曲库和元数据更新场景中，两个任务是最快的默认值：
单任务中位耗时 3.62 秒、双任务 2.99 秒、四任务 3.39 秒、八任务 5.66 秒。同时进行更多
解密、复制和标签写入可能引发存储竞争，所以并发越高并不一定越快。你可以针对不同硬盘
或负载自行调整：

```sh
unlockm ~/Music/Encrypted -o ~/Music/Unlocked --jobs 4
```

`--metadata-jobs` 是另一个独立上限：它限制并发元数据网络请求数，而不是文件转换任务数。

## 元数据与隐私

不设置 `--update-metadata` 时，unlockm 不会发起元数据网络请求，并会保留内嵌标签和
封面。启用该选项后，程序可使用 NCM 容器信息和经过验证的 QMC 身份补全元数据。如果
远程结果缺失、有歧义、无效或不可用，已有信息优先保留。

元数据查询或标签写入失败只会产生警告，不会使解密失败。封面在使用前会检查大小、文件
特征和可解码性。请求只允许 HTTPS，并限定到文档列出的 QQ/网易云 API 与封面主机；
本地地址、IP 字面量、userinfo 和未知主机会在发送请求前被拒绝。

## 支持的格式系列与密钥

| 系列 | 说明 |
| --- | --- |
| NCM | 网易云音乐容器，包括内嵌元数据和封面。 |
| QMC / QQ / Moo | QMC v1/v2 及其别名。QMC v2 优先使用文件尾内嵌密钥，再尝试配置的密钥来源。 |
| KGM / KGG / KGMA / VPR | 酷狗格式。KGM v5 可能需要酷狗加密密钥数据库。 |
| KWM | 酷我音乐格式。 |
| Xiami | 虾米加密音频。 |
| Ximalaya | Android 和 PC 的 X2M/X3M 变体。 |
| TM | QQ 音乐 iOS TM 变体。 |

QMC v2 在内嵌文件尾密钥之后，按以下顺序尝试密钥选项：

1. `--qmc-key-map` — JSON 对象或多行 `identifier=ekey`。
2. `--qmc-mmkv` — QQMusic MMKV 存储；加密存储需同时提供 `--qmc-mmkv-key`，并且
   对应的 `.crc` 文件必须存在。
3. `--qmc-ekey` — 一个显式的兜底 ekey。

在 macOS 上，如果常规位置存在 QQMusic 8.x MMKV 或 10.x plist 派生存储，unlockm
也会自动发现。KGM v5 可通过 `--kgg-db` 指定数据库，并可发现 Windows 上的常规数据库
位置。秘密值只用于构建内存中的密钥映射，绝不会写入日志。

具有已注册加密扩展名的文件还必须通过格式校验。损坏或缺少密钥的加密输入会按文件报告
失败，不会被误当作普通文件复制成密文。

## 监视模式

`--watch` 会先完成初始目录转换，然后递归监视稳定的文件变化：

```sh
unlockm ~/Music/Encrypted -o ~/Music/Unlocked --watch --overwrite
```

创建、修改、重命名和删除事件风暴会被合并。如果文件在处理期间再次变化，unlockm 会
安排一次干净的后续处理。按 Ctrl-C 或收到删除事件时，程序会取消私有临时工作，不会
删除源文件，也不会发布不完整结果。

## 退出状态与故障排查

- 退出码 `0`：所有文件均已完成、复制或被安全跳过。
- 退出码 `1`：命令或整体运行无法启动或继续。
- 退出码 `3`：部分文件失败或冲突；未受影响的文件仍已继续处理。

常见情况：

- **所有文件都被跳过：** 目标文件已经存在；只有确实需要替换时才使用 `--overwrite`。
- **加密文件校验失败：** 确认文件未损坏，并确认所需的 QMC/KGM 密钥来源可用。
- **出现元数据警告：** 已解密输出仍会保留；元数据补全本来就是尽力而为。
- **重定向输出时没有动画：** 这是预期行为；只有确实需要终端样式输出时才使用
  `--progress always`。
- **更快的硬盘表现不同：** 更改默认值前，请在有代表性的曲库副本上测试
  `--jobs 1`、`2` 和 `4`。

## 安全模型

解密与复制使用有界缓冲流式进行。程序会在重负载工作开始前拒绝目标冲突，保证输出路径
不越界，并且只有同目录下的私有临时文件完整后，目标文件才会变为可见。发布前会再次
检查源文件大小和修改时间；元数据恢复也绝不会用部分修改的文件替换有效的已解密音频。

## 桌面端在线元数据搜索

桌面端默认使用 MusicBrainz 搜索：它面向非商业使用免费、无需账号或 API key，且只会在用户
明确发起搜索后联网。UnlockM 会发送有意义的 User-Agent，并将 MusicBrainz 请求限制为每秒
一次。TheAudioDB 使用公开 `123` key 查找封面，但仅在点击“使用封面”或“全部填入”后进行，
不会自动下载。商业或高流量的 MusicBrainz 使用需要另行采用 MetaBrainz 服务或取得适当协议。
两个免费服务均只能由 Rust 访问固定主机，使用有界、禁止重定向的请求，webview 不会收到远程 URL。

Spotify 是需要 Premium 的可选 secondary provider；桌面端元数据编辑器仅在用户明确点击后才搜索
Spotify，打开应用或编辑器不会联网。配置
公开的构建期 `UNLOCKM_SPOTIFY_CLIENT_ID`，并在 Spotify Dashboard 注册
`http://127.0.0.1/callback`。桌面 OAuth 使用 PKCE，绝不使用 client secret；未配置 ID
时编辑器仍可使用，只会显示搜索不可用。

refresh credential 只保存在系统安全凭证库（macOS Keychain、Windows Credential
Manager）。access token、OAuth code、PKCE 值、搜索与 catalog response 不会写入 SQLite、
浏览器存储、遥测或日志。点击“断开 Spotify”会删除本地 credential；Spotify 授权本身请在
Spotify 账号的 Apps 页面管理。请在当前约六个月 refresh-token 生命周期结束前重新授权。

Spotify Development Mode 要求 Premium app owner，当前最多允许 5 位 allowlist 用户；公开生产
启用需取得 Extended Quota（或明确作为 allowlist 预览），并由项目所有者确认 Spotify 政策。
结果始终带来源与 track 链接。专辑封面只会在用户选择后下载，
仅存在于短期 Rust handle 和当前编辑草稿，且仍只有既有“保存”会写文件。UnlockM 不缓存
Spotify catalog、不下载 audio preview，也不发送遥测。

## 许可证与声明

项目新增代码采用 MIT 许可证。复制代码和测试夹具的来源说明见 [LICENSE](LICENSE) 与
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
