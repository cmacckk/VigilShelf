# VigilShelf — Windows 版 Folder Hub 实施方案

参照对象：<https://www.finderhub.app/>（Folder Hub，macOS）

## 1. 一句话目标

鼠标移到屏幕顶部边缘就滑出一个文件面板，移开自动收起；用「工作空间」绑定常用目录，从而在**不打开资源管理器**的前提下，把项目文件直接拖到当前正在用的程序里。

## 2. 成功标准（可验证）

| # | 场景 | 期望结果 |
|---|------|----------|
| 1 | 任何时候把鼠标移到屏幕顶边并停留 | 面板在该屏幕顶部居中滑出，耗时 < 300ms |
| 2 | 鼠标离开面板 | 面板自动收起，不残留、不抢焦点 |
| 3 | 双击面板里的文件夹 | 进入该目录，面包屑同步更新，可点面包屑回退 |
| 4 | 从面板拖一个文件到记事本 / 微信 / 资源管理器 | 对方正常接收该文件 |
| 5 | 在文件上右键 | 出现菜单：打开 / 在资源管理器中显示 / 用其他程序打开 / 复制 / 粘贴 / 移入回收站 / 复制路径 |
| 6 | 全屏程序（游戏、F11 视频）盖住顶边时按全局快捷键 | 面板在光标旁弹出，全屏程序不受影响 |
| 7 | 两个显示器，鼠标在副屏顶边停留 | 面板只在副屏顶部弹出 |

## 3. 技术选型

- **外壳**：Tauri 2（Rust 后端 + WebView2 前端），安装包几 MB
- **前端**：React 19 + TypeScript + Vite，纯 CSS（不引 UI 框架）
- **文件操作**：`std::fs` + `trash`（回收站）+ `clipboard-win`（CF_HDROP 文件剪贴板）
  加 `drag`（原生 IDropSource 拖出）、`dirs`（用户目录）
- **窗口形态**：无边框 + 透明 + 置顶 + 不占任务栏 + 启动隐藏

## 4. Windows 与 macOS 的差异处理（等价替换原则）

| macOS 版做法 | Windows 版做法 | 判定 |
|---|---|---|
| 鼠标移到刘海呼出 | 屏幕顶边 1px 热区 + 停留 150ms | 等价替换 |
| ⌘+⌥+→ 光标旁呼出 | Ctrl+Alt+Space 在屏幕顶部居中呼出（可改） | 等价替换 |
| 空格 Quick Look 预览 | 暂不实现（MVP 范围外，二期用 WebView2 内嵌预览） | 暂缺，已登记 |
| Control+点击 AirDrop | 移除（Windows 无对应能力） | 功能删除，写入 CHANGELOG |
| Cmd+点击 在 Finder 中显示 | Ctrl+点击 在资源管理器中显示 | 等价替换 |
| 移至废纸篓 | 移至回收站 | 等价替换 |
| 多显示器 | 多显示器（按光标所在屏定位） | 一致 |
| 面板可自由拖动 | 位置固定为当前显示器顶边居中，不可拖动 | 功能删除，写入 CHANGELOG |

**已知冲突与取舍**：
1. Windows 11 最大化按钮悬停会弹「贴靠布局」，任务栏也可能被用户放在顶部 → 热区只在屏幕**顶部 1px**、且检测到任务栏在顶部时热区让位；始终保留全局快捷键兜底。
2. 全屏应用会盖住顶边 → 热区失效属预期行为，快捷键兜底。

## 5. 分阶段任务与验证方式

### P0 骨架
- 工程更名 VigilShelf，主窗口：无边框 / 透明 / 置顶 / 不占任务栏 / 启动隐藏
- 托盘图标：左键切换显隐，右键菜单（显示 / 设置 / 退出）
- 全局快捷键 Ctrl+Alt+Space
- **验证**：`pnpm tauri dev` 启动后窗口默认不可见；点托盘出现；快捷键在光标所在屏弹出；Esc 收起

### P1 文件能力
- Rust 命令：`list_dir` / `open_path` / `reveal_in_explorer` / `trash_path` /
  `show_open_with_dialog` / `list_quick_locations`
- 前端：文件列表（图标、名称、大小、修改时间）、面包屑、双击进入、后退、隐藏文件开关
- **验证**：`cargo test` 覆盖 `list_dir`（目录优先排序、隐藏文件过滤、路径不存在时报错信息明确）

### P2 工作空间
- 模型 `{id, name, path}`，存 `appDataDir/workspaces.json`
- 首次启动写入默认空间（桌面 / 下载 / 文档 / 图片）
- 前端 tab 切换 + 添加（选目录）+ 重命名 + 删除
- **验证**：重启后空间保留；`workspaces.json` 缺失或为空数组时回落到默认空间；损坏 JSON 不 panic

### P3 边缘热区
- 独立线程每 60ms 轮询光标位置（Tauri `cursor_position`）
- 隐藏态：光标进入顶部触发带并停留 `hotzone_delay_ms` → 在该显示器顶部居中显示
- 显示态：光标离开窗口矩形（含 8px 缓冲）超过 300ms → 隐藏
- **验证**：副屏顶边停留只在副屏弹出；把窗口拖到第二屏后热区跟随；热区开关生效

### P4 交互闭环
- 拖出：**Rust 命令 `drag_out`**，内部用 `drag` crate 走 OLE `IDropSource`。
  ⚠️ 原方案写的是前端 `getCurrentWebview().startDrag({item: paths})`，但 **Tauri 2 的 `Webview`
  类并没有 `startDrag` 方法**（tsc 报 TS2339），HTML5 拖放又无法向其他进程交付真实文件路径，
  所以改为 Rust 侧原生拖拽。失败时前端回落为「复制路径」。
- 右键菜单：自定义 React 菜单（比系统菜单可控，样式统一）
- 拖入：`onDragDropEvent` 把外部文件拖到列表中的文件夹行上 → 复制 / 移动
- 文件剪贴板：`clipboard-win` 写/读 CF_HDROP，粘贴时复制到当前目录
- **验证**：拖到记事本成功；复制后在资源管理器 Ctrl+V 能粘贴出该文件

### P5 验收与打包
- Rust：`cargo fmt --check`、`cargo check`、`cargo clippy --all-targets`、`cargo test`
- 前端：`pnpm exec tsc --noEmit`、`pnpm build`
- 打包：`pnpm tauri build --bundles nsis`（NSIS 安装包）
- 文档：README（安装、快捷键、功能对照表）+ CHANGELOG

### P6 外观与固定位置（0.2.0 追加）
- **面板不可拖动**：删掉顶栏 `data-tauri-drag-region`；位置固定为当前显示器顶边居中
- 删掉 `anchor_to_cursor` / `CURSOR_INSET` 与命令 `show_window_at_cursor` / `anchor_to_cursor`，
  快捷键与托盘统一走 `window_ctl::toggle_panel`
- `Settings` 新增 7 个外观字段（theme / fontFamily / fontSize / backgroundImage /
  bgBlur / bgDim / bgOpacity），`Settings::sanitized()` 在加载时夹回越界值，
  旧配置文件直接升级不重写
- 命令 `list_system_fonts`：走 PowerShell 读注册表（**不用 `reg.exe`** —— 它按控制台代码页输出，
  中文系统上读回 Rust 会乱码），`OnceLock` 缓存，失败回落内置字体表
- 背景图走 Tauri `protocol-asset`（`convertFileSrc`），白名单只列图片扩展名；
  独立 `.backdrop` 层承载 `filter: blur()`，因为 `filter` 会连子元素一起模糊
- 外观集中由 `src/lib/appearance.ts` 写 `:root` CSS 变量；尺寸用比例推导而非写死 px
- 动画只动 `transform` / `opacity`，并挂 `prefers-reduced-motion` 开关
- **不做退场动画**：热区收起由 Rust 侧直接 `hide()`，只给部分路径做退场会让行为不一致
- **验证**：新增 `sanitized()` 与字体族解析共 16 项单测；`cargo test` 32 项全绿；
  `pnpm exec tsc --noEmit` + `pnpm build` 干净

## 6. 约束（来自用户）

- 不改目标系统的任何数据、配置、密码；不做增删改数据的操作
- 不关闭任何接口、服务器、设备
- 本工具对文件系统的写操作仅限用户显式点击的动作（移入回收站、粘贴、拖入复制），且均可在资源管理器中撤销

### P7 面板尺寸改为常量（0.2.1 追加）

用户反馈「界面太大」。原实现把 `windowWidth` / `windowHeight` 存在 `Settings` 里，导致面板尺寸有
**三个**来源：`tauri.conf.json` 的初始值、`Settings::default()`、以及磁盘上已经存在的 `settings.json`。
磁盘旧值优先，所以把默认尺寸改小之后用户完全看不出变化 —— 这类「改了没生效」最难排查。

改动：

- 删除 `Settings::window_width` / `window_height`（旧 `settings.json` 里的同名字段被当作未知字段忽略，
  有测试守住）
- 新增 `window_ctl::PANEL_WIDTH = 560` / `PANEL_HEIGHT = 380`，成为唯一来源
- `window_ctl::resize(window, &settings)` → `window_ctl::apply_panel_size(app)`，不再读设置
- `anchor_to_top(app, &settings)` → `anchor_to_top(app)`（它本来也不需要设置）
- 新增测试 `tauri_conf_agrees_with_panel_size`：直接读 `tauri.conf.json` 比对，防止两个常量漂移
- 顺带删掉热区线程里每次触发都读一遍磁盘设置的 `store::load_settings` 调用

尺寸取值：`PANEL_WIDTH = 560`、`PANEL_HEIGHT = 270`。275 是按「列表区正好显示 5 行」反推的
（默认字号 13px；公式相加得 275，实测多出约 4.5px 的第 6 行，取 270 并用截图核对）。
各项倍数定义在 `src/lib/appearance.ts` 的 `sizeVars` 里，**改那边必须回头改这里**。
已知边界：只在默认字号下严格 5 行；字号调大会同时撑高行高与顶栏，可见行数变少。

### P8 浅色主题可用性修正（0.2.1 追加）

用户反馈：「切换浅色主题的时候文字几乎看不见，主题也不对」。查下来是**三个独立的问题叠在一起**，
其中两个是根因、一个是一颗隐藏的雷：

1. **浮层底色没跟着主题走**（浮层深底 + 深色文字）。
   `.panel`（设置面板 / 确认框 / 添加工作空间）底色写死 `#232329`，
   `.menu`（右键菜单）写死 `#26262c`，而里面的文字用的是 `var(--text)` ——
   切到浅色主题后 `--text` 变成 `#1c1c1e`，深底深字，于是「文字几乎看不见」；
   同时因为浮层还是深色，整体看上去就是「主题没生效」。
   同一类漏改的还有：遮罩、输入框底色、凹陷区底色、阴影、滚动条滑块、禁用态、
   危险提示，共 20 多处。修法是全部抽成主题变量（`--surface` / `--scrim` /
   `--field-bg` / `--well-bg` / `--toast-bg` / `--shadow-*` /
   `--scroll-thumb*` / `--disabled` / `--danger-*` / `--sel-bg` /
   `--drop-*` / `--on-accent`），深/浅两套各自定义。
   **约定：颜色只能出现在 `:root` 与 `[data-theme="light"]` 两块里，别处一律 `var(...)`。**
2. **背景图遮罩用的是黑色**（浅色主题下把黑字压得更黑）。
   `.backdrop::after` 写死 `rgba(0, 0, 0, var(--bg-dim))`。深色主题是白字，盖黑是对的；
   浅色主题是黑字，越盖越看不清。改为 `rgba(var(--dim-rgb), var(--bg-dim))`，
   `--dim-rgb` 随主题变（深 `0,0,0` / 浅 `255,255,255`）；滑杆标签在浅色主题下显示「提亮」。
   另加 `LIGHT_DIM_FLOOR = 55`：浅色主题的遮罩强度取 `max(用户值, 55)`。
   不给地板的话，用户把滑杆拉到 0 时第一眼是一片读不出来的字 —— 把可用性押在「用户会调参」上。
3. **`fontStack(undefined)` 会抛错，而它跑在 `useEffect` 里**，于是整个 App 白屏，
   React 只报「An error occurred in the App component」，看不出是字段缺失。
   踩到它的场景很具体：用陈旧的 `target/debug/vigilshelf.exe`（它的 `Settings` 结构里还没有
   `fontFamily`）配新前端。现在外观字段一律 `?? 默认值` 兜底。

附带调整：12 组文件图标颜色是照深色底挑的浅彩色，白底上发飘，
浅色主题单独收了一档明度（只换颜色，形状与尺寸共用）。

验证方式与取证：`scripts/smoke-shot.ps1` / `scripts/click.ps1` 截图比对。
顶栏四个按钮的字符中心在物理像素 x ≈ 1530（主题）/ 1576（隐藏文件）/ 1621（设置）/ 1665（收起），
y ≈ 32（150% 缩放下）—— 点偏一格会点到隔壁按钮（曾经点到「隐藏文件」，把 showHidden 改掉了）。
验证用「全局主题」的方式是**临时**把 `%APPDATA%/com.mechrevo.vigilshelf/settings.json` 的
`theme` 改成 `light`，截完图再从备份恢复，不留副作用。

### P9 文件列表改为卡片网格（0.3.0 追加）

用户反馈：「界面太丑陋了 美化界面 工具制作为卡片式的」。因 VigilShelf 里没有「工具」模块，
先回问清范围，用户选定三件事：**文件列表改成卡片网格**、
**保持 270 高、4 列 × 2 行 = 8 个小卡片**、**延续毛玻璃、做精致**。

改动：

- `src/components/FileList.tsx`：`.row` → `.card`，卡片内容为「图标 / 文件名 / 大小」三层
- `src/App.css`：`.filelist` 从普通块改为 `display: grid` + `repeat(4, minmax(0, 1fr))`；
  新增 `.card` 系列样式（悬停浮起、选中外发光、拖入高亮、逐张渐入 `--card-index`）
- `src/lib/appearance.ts` 的 `sizeVars()`：去掉 `--row-h` / `--icon-col` /
  `--size-col` / `--time-col`（都是列表视图专用，已无引用），
  改为 `--card-gap` / `--icon-size`
- `src-tauri/tauri.conf.json` 不变（面板仍是 560×270），**Rust 侧零改动**

两个关键工程决定：

1. **行高由容器高度反推，不写固定值**。`grid-auto-rows: calc((100% - var(--card-gap)) / 2)`
   让「两行 + 行距」正好等于列表区高度，第 3 行自动落在可视区之外。
   第一版写的是固定 `--card-h: 64px`，结果第 3 行露出约 10px 的卡头顶
   —— 与 0.2.1 里「第 6 行漏出 4.5px」是同一类问题，只是换了个位置重现。
2. **图标倍率不能跟着字号线性放大**。行高是被容器高度除出来的、几乎不随字号变，而文字和图标会。
   倍率 1.55 时字号调到 16，内容就溢出卡片、被下一行的卡片盖住，看起来像字体渲染坏了；
   收到 1.35 后 16px 字号下仍有余量（`.card` 的 `overflow: hidden` 只是兜底）。

顺带清掉的债务：删除 `src/lib/format.ts` 的 `middleEllipsis()`。它和 CSS 省略号叠在同一行上，
短卡片里会出现「Cherry S…dio…」这种**两个省略号**；而且它只被 FileList 用。文件名截断改由
CSS 接管（按像素算，比按字符数算准）；代价是扩展名可能被截掉，文件类型改由图标表达。

验证：`pnpm exec tsc --noEmit` 干净，`pnpm build` 成功；截图在 **深色 13px** 与 **浅色 16px**
（临时改 `settings.json`，截完从备份逐字恢复）两种极端下各拍一张，
都是一屏 4×2、没有第三行残留、文字不被裁切。

### P10 外壳真图标与进退场动画（0.4.0 追加）

问题一：卡片上的图标是手绘的通用图形，`.lnk` 一律显示同一个「页」形状，跟资源管理器对不上。

- 手绘 SVG 只能表达「类型」，表达不了「这个程序自己的图标」。要跟资源管理器一致，
  只能问外壳要图。`SHGetFileInfo` 只给 16 / 32px，在 125% / 150% 缩放屏上放大后是糊的；
  最终用 `file_icon_provider`（内部走 `IShellItemImageFactory::GetImage`，可取任意尺寸）。
- 它自带的缓存策略正好合用：**按扩展名缓存，`.exe` / `.lnk` / 无扩展名永不缓存**
  —— 这两种文件的图标是文件自带的，不是格式通用的。所以「一个目录 500 个文件」
  实际只有十几次真正的外壳调用。
- 图标以 PNG data URL 走 IPC：直接传 RGBA 会因为 JSON 编码膨胀成数字列表，比 base64 还大一圈。
- 取不到的路径**不放进返回值**（而不是给空串），前端据此保留 SVG 兜底，
  也顺带把「这个路径没有图标」记住不再重问。
- 命令声明成 `async`：取图标要走外壳，冷启动首次可能几百毫秒，同步命令会冻住界面。

问题二：只有第一次启动能看到入场动画。

- `animation` 只在元素创建时执行一次，而面板是 hide/show、React 树从不卸载 ——
  第二遍起就没有动画了。修法是每次弹出换一次 `key`（`App.tsx` 的 `runId`）强制重建 `.app`。
- 顺带把退场动画补上。退场只能在 webview 里放（窗口一旦 `hide()` 就什么都看不见了），
  所以收起改成两段：Rust 发 `vigilshelf://panel-hiding` → 前端放 150ms 动画 →
  前端回来调 `hide_window`。Rust 侧留一个 600ms 的兜底线程，前端卡住时直接隐藏，
  否则面板会永远留在屏幕上、只能杀进程。
- 「一次收起只发一次事件」用 `HotZoneState::hide_pending` 的原子 CAS 保证：
  热区每 60ms 就问一次要不要收起，而前端从收到事件到真正隐藏有 170ms，
  不加这个标志会连发好几次。同一个标志兼作兜底线程的判据。
- 四条收起路径（热区 / 全局快捷键 / Esc / ✕）统一走前端的 `beginClose()`，
  否则「热区收起有动画、Esc 收起硬切」，一眼就能看出不一致。

### P11 项目改名 VigilShelf 与发布陷阱（0.5.0 追加）

改名本身：一条 `sed` 批量替 18 个文件，**替换顺序必须先长后短**
（`reachfile_lib` → `reachfile-tray` → `reachfile-hotzone` → `reachfile://` →
`com.mechrevo.reachfile` → `ReachFile` → `reachfile`）。反过来先把 `reachfile` 换掉，
会把前面几个长串拆成四不像。

改名必然丢配置 —— 配置目录名是从 identifier 推出来的，`com.mechrevo.reachfile` 会变成
`com.mechrevo.vigilshelf`。所以 `store.rs` 加了一次性迁移：

- `LEGACY_DIR_NAME` + `MIGRATION_DONE: OnceLock<()>`。`config_dir()` 会被反复调用，只能迁一次。
- **新目录里已有任一配置文件就跳过迁移**：用户可能已经在新版里改过设置，拿旧文件盖回去
  比丢失更难排查。
- **只读旧目录、只写新目录**，老文件原地保留（万一迁移出错还能手动找回来）。

发布陷阱（本次真实事故，值得反复看）：

- 起因：`link.exe` 报 `0xc0000142`（STATUS_DLL_INIT_FAILED），我退回用普通
  `cargo build --release` 救急。**不走 Tauri CLI 的 `cargo build --release` 产出的是 dev 构建**：
  它不内嵌 `dist/`，而是去连 `tauri.conf.json` 的 `build.devUrl` = `http://localhost:1420`。
  当时另一个项目的 Vite 正跑在 1420 上，于是我自己的面板老老实实渲染了**别人的界面**，
  被误判成「代码被污染」。
- 判定链（每一环都是实测）：时间戳（安装包 23:45 / exe 23:57，安装包做完之后又被一条命令
  覆盖了 exe）→ `[::1]:1420` 上有一条 ESTABLISHED 连接，换回 Tauri CLI 重建后消失 →
  WebView2 缓存目录里 `grep -a` 到对方界面的关键字、搜不到自己的 →
  `PrintWindow(hwnd, hdc, 2)`（让窗口画自己，**完全无视 z 序**）画出的也是对方界面 →
  用 Tauri CLI 重建后 `PrintWindow` 画出的就是自己的界面。
- 教训一：`cargo build --release` ≠ 发布产物。若 Tauri CLI 打包因 link.exe 失败，
  **不要用 `cargo build --release` 顶替**，要查 link.exe 本身（`Start-Process -NoNewWindow`
  包着跑会触发它，前台跑就正常）。
- 教训二：`PrintWindow(hwnd, hdc, 2)` 是无视 z 序的取证手段，能区分「被别的窗口盖住」和
  「窗口自己画的就是错的」。`CopyFromScreen` 受 z 序影响，两者同时指向同一结论才算证据。
- 教训三：**像素值看着像自己的界面不够**。对方也是深色 UI，随便一个深色背景都能凑出相似的
  RGB —— 曾经把对方的深色工具启动器误读成「我的面板」好几轮。只有图里能认出自己的控件
  （面包屑、卡片、状态栏）才算证据。
- 防复发：`lib.rs` 的 `run()` 开头加了 `if tauri::is_dev() { eprintln!(...) }` 启动告警。

顺手修的两个 UI 缺陷：右键菜单在 270px 高的面板里被裁掉（除「移入回收站」外都看不见）→
菜单项内边距 6px → 5px、菜单加 `max-height: calc(100vh - 12px)` + `overflow-y: auto`；
面包屑横向滚动后左端露出一截孤零零的 `›` → 滚动位置 > 4px 时给左侧加渐隐遮罩。

README 重拍截图的做法：`docs/demo/` 里造一个演示目录（**别把真实文件名拍进公开 README**）；
`scripts/shots.ps1` 用 `PrintWindow` 抓图。两个硬约束：

- **不要把面板挪到别处再截**。Chromium 对「被遮住的 WebView2」有遮挡检测，一旦判定被遮住
  就停止产帧，`PrintWindow` 于是画出一片纯黑。面板必须在它自己的顶边锚点位置、且在最上层。
- **`Ctrl+F` 后只发一个按键**。中文输入法会在两个音节开头之间插入撇号（`m` + `p` 变成 `m'p`），
  截图里的筛选词就废了。
