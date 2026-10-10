# PhotoCraft 鸿蒙原生自绘实验验证记录

日期：2026-10-09。分支：`codex/hmos-native-selfdraw`。包名及版本沿用 `io.github.storytold.photocraft.hmos` / `0.5.0.1`。没有发布 release 或合并 main。

## 实现范围

原有 Rust 文档、画笔、历史、图层及格式引擎未改动。共享 UI 的修改限于宿主帧接口、可选 eframe、GPU 类型直接引用、宿主语言入口及 URI 平台的保存/快速导出服务选择。桌面 eframe 实现委托同一组帧接口。所有新的 FFI、NativeWindow 所有权和 GLES 渲染代码在 subtree 外。

原生调用链为 XComponent → NativeWindow → wgpu GLES/EGL → egui-wgpu。文档由 CPU 合成。单个 Rust 工作线程拥有应用、输入队列、egui context 和 GPU；ArkTS 负责系统选择器、异步 URI 文件 I/O、外链和应用窗口服务。Save 与 Save As 每次重新选择目标，选定名称用于格式判断，URI 留在 ArkTS 映射内。写入、fsync 和关闭全部成功后才返回保存成功。

## 编译与自动检查

| 项目 | 结果 |
|---|---|
| ARM64 Rust release + 原生库链接 | 通过 |
| x86_64 Rust release + 原生库链接 | 通过；本轮未运行 x86_64 模拟器 |
| debug HAP 构建、签名、安装 | 通过；签名仅在本地 |
| HAP 双 ABI ELF / 无 HTML、JS、WASM 检查 | 通过 |
| 原生普通依赖树 | 无 eframe、winit、rfd、glutin |
| 原生适配层单元测试 | 6 通过，覆盖取消唤醒、坐标/指针释放、修饰键、快捷键文本、字体初始化与未保存关闭检查顺序 |
| 共享 UI 单元测试 | 883 通过，3 个原有忽略项；增加宿主绘制、取消/权限拒绝/关闭失败保存、重选及改名保存检查 |
| UI 全目标 clippy / OHOS 适配层 clippy | `-D warnings` 通过 |
| `cargo metadata --offline` | 通过 |
| `cargo xtask layers` | 28 crates，无层级违规 |
| `cargo xtask wasm` | 通过，包括 heif 特性检查 |
| Trunk Web 构建 | 通过，保留原有两项菜单 dead_code 警告 |
| `cargo xtask perf --quick` | 完成；small-document quick 模式不应用完整尺寸预算，不作为 Pad 性能验收 |
| 脚本语法 / Git diff 空白检查 | 通过 |

本机终端的 `NO_COLOR=1` 与 Trunk 当前布尔参数不兼容，Web 回归命令使用 `env -u NO_COLOR trunk build --skip-version-check`，在 `upstream/photocraft/apps/photocraft-web/` 运行。

## ARM64 模拟器实测

HarmonyOS 6.0 / API 20，`PhotoCraftPadAPI20`，hdc `127.0.0.1:5555`；屏幕 2880×1920，XComponent 2880×1786，系统缩放 2，中文、深色主题。模拟器已恢复连接；显示实际 PhotoCraft 编辑器，没有 Web 或示例三角形入口。

| 操作 | 结果 / 证据 |
|---|---|
| 完整 UI、中文字体和菜单 | [欢迎页](docs/native/native-welcome.jpeg)、[最终 HAP 编辑器](docs/native/native-final.jpeg) |
| Ctrl+N 新建 1920×1080 RGB/8 文档 | [新建对话框](docs/native/native-new-dialog.jpeg) |
| 触摸画笔、第二层、两条笔迹 | [编辑画面](docs/native/native-cancel-save.jpeg) |
| Ctrl+Z / Ctrl+Shift+Z | 撤销移除第二条笔迹，重做恢复；[撤销](docs/native/native-undo.jpeg) |
| 取消系统保存 | 星号、图层和笔迹保留；[截图](docs/native/native-cancel-save.jpeg) |
| Ctrl+Shift+S Save As | 命名 PSD 仍进入系统选择器，取消后原文档保持已保存；[选择器](docs/native/native-save-as.jpeg) |
| PSD 保存、系统改名 | `native-selfdraw.psd` 写入下载目录，状态栏记录实际名称，星号消失；[选择器](docs/native/native-save-picker.jpeg)、[已保存](docs/native/native-saved.jpeg) |
| 系统 URI 读取并重开 PSD | 新标签，两层与两条笔迹保留；[重开](docs/native/native-reopen-psd.jpeg) |

| PNG 快速导出 | 通过系统选择器写入 `native-selfdraw.png`，报告预期的图层合并警告；[导出](docs/native/native-exported-png.jpeg) |
| PNG 重开 | 1920×1080、单层，两条笔迹与 PSD 一致；[重开](docs/native/native-reopen-png.jpeg) |
| 取消系统打开 | 原有三个标签和文档保留 |

| 前后台重复切换三次 | 三个标签及当前 PNG 画面保留，无额外笔迹 |
| 横屏→竖屏→横屏 | 窗口尺寸更新后 UI、字体与当前文档保留；[竖屏](docs/native/native-rotate.jpeg)。沿用原布局，竖屏下画布/标签会有裁切，未重做平板布局 |

| Ctrl+Q 未保存提示 | 最终包显示原有未保存确认；[提示](docs/native/native-close-prompt.jpeg)。发现并修复了 egui 内置关闭命令绕过首次检查的宿主边界，回归测试验证 Close→viewport close event→编辑器 veto 的顺序 |

| 退出提示选择保存→取消系统选择器 | 应用没有退出，脏状态和两个图层保留，未保存提示继续等待；[截图](docs/native/native-close-save-cancel.jpeg) |

| 退出提示选择保存→完成写入 | 选择器期间未提前退出；确认保存后回到系统桌面。日志：19:36:16.766 发起 `write native-close.psd`，19:36:16.788 发出 `close`，关闭发生在 ArkTS 写入/fsync/close 回复之后 |

## 验证边界

已在模拟器验收基本单指输入、键盘快捷键、选择器取消、编辑/格式闭环和退出保存。不可写 URI 的设备故障注入、运行中 GPU device loss、强制 surface detach/rebind、鼠标/滚轮真外设以及系统窗口关闭按钮仍需专门验收；实现已有写入失败保留脏状态、surface 丢失重建和取消唤醒路径。指针取消/修饰键、错误保存及未保存关闭顺序由单元测试覆盖。旋转实测证明尺寸变化和纹理保留，不等同于 GPU device loss 测试。

当前 Open Recent 条目只保存展示名称，不能作为沙箱普通路径读取；请用 File→打开的系统选择器重新打开。完整 Recent/URI 持久授权需要后续补齐，未把 URI 当普通文件路径。

## 视觉对比

已在替换入口前保留 [ArkWeb 基线](docs/native/arkweb-baseline.jpeg)，并查看同一模拟器/分辨率上的原生欢迎页、新建窗口、菜单、工具栏、图层和画布截图。布局、主题、图标及字号保持原配置，中文使用系统 HarmonyOS Sans SC/TC，拉丁字体沿用上游。没有承诺逐像素一致；截图 JPEG、系统时间、指针位置和内容状态会影响像素差异。真实 Pad 的字体/DPI、GLES 驱动、压感和性能尚未验收。

## 交付与复现

```sh
rustup target add aarch64-unknown-linux-ohos x86_64-unknown-linux-ohos
scripts/dev.sh build
scripts/dev.sh run
python3 scripts/verify_native.py entry/build/default/outputs/default/entry-default-unsigned.hap
```

Unsigned HAP SHA-256：`a5227eaf7b55ad12a964b294bbf7782f1df67f68b887bfcb8a010613a9ede82a`。

本地 HAP：`entry/build/default/outputs/default/entry-default-signed.hap` 与 `entry-default-unsigned.hap`（忽略、不提交）。Rust 修改需要重建部署；CI 手动任务先构建 Rust 双架构再打 unsigned HAP。源码、锁文件、脚本和验收截图保留在实验分支。

## 2026-10-09 输入桥接回归

已接入原生触点工具类型、压感、倾角及橡皮擦来源，并复用 HarmonyOS 手写笔双击/长按事件适配器。共享 StylusFeed 测试覆盖手指/笔区分、压感、双击切换和长按菜单；真实 Pad 的笔事件与压感曲线仍需实机验收。

系统输入法现已绑定 egui 输入框和画布文字编辑会话。模拟器验证：Help 一次打开后点击搜索框稳定弹出键盘、菜单保持打开、候选中文写入搜索框；文字工具点击画布后弹出键盘，候选中文写入文字图层。软键盘弹出时整个 XComponent 缩小并重排，底部画布文字光标通过视图滚动保持可见。焦点异步切换、组合输入、UTF-16 选择/删除和过期回调由原生及 ArkTS 测试覆盖。

本轮：原生 Rust 10 项通过，ArkTS 输入法和手写笔适配测试通过，双架构构建、HAP 校验、Clippy、依赖分层、离线 metadata 和 Trunk 构建通过。上游 UI 测试 883 项通过、3 项忽略；1 项快捷键测试因当前环境无法创建 GPU 适配器失败。上方 SHA-256 属于先前验收包，本轮 HAP 已重新构建。

后续阶段：系统剪贴板、手写笔悬停与多点手势、自动恢复、超过 256 MiB 文件与大图优化、真实 Pad 输入及性能验收。

## 2026-10-10 官方 v0.5.0 菜单对比

按用户要求，`menus.rs` 已完整恢复为官方 v0.5.0，blob 校验为 `c371518e3149ccd4e7e5afa8bc04c57ad44c9b69`；撤回 ArkWeb 点击打开分支及此前本地 Help 焦点/关闭补丁，保留独立原生输入法桥接。

双架构 HAP 已重建并安装到模拟器。单次点击 File 和 Help 均能稳定展开，未复现 ArkWeb 的菜单打开后立即关闭。Help 搜索问题仍复现：打开菜单后搜索框失去自动焦点，未保持键盘打开；点击搜索框会关闭菜单。因此普通菜单打开与 Help 输入是两个独立问题，不能把恢复官方菜单视为完成 Help 自动输入法修复。该对比结果保留作历史记录，后续 Help 修复见下文。

官方菜单 11 项测试、原生桥接 11 项测试、ArkTS 输入法测试、Clippy、分层检查、离线 metadata、Trunk 和 HAP 校验通过。安装前已用系统选择器另存当前两层文字测试文档到模拟器下载目录，未覆盖原文件。

## 2026-10-10 移除 Web 开发模式

已移除 ArkWeb 开发页面、冷启动模式选择、Web 路由、Trunk serve/launch-web/web 命令、网页手写笔 JavaScript 注入及旧离线网页资源校验脚本。`scripts/dev.sh` 仅保留原生 build/run/launch；文档与代理开发说明同步更新。上游 subtree 的 Web 源码和完整历史保留，不参与鸿蒙构建开发流程。

启动/生命周期、主题和手写笔适配测试通过，删除的 Web 命令均以错误码 2 拒绝执行。Rust 双架构与 HAP 构建通过，包内仅原生页面且没有 WebDev、ArkWeb 导入或本地开发 URL。模拟器实测以旧 `photocraft.dev=true` 参数冷启动仍进入 Native egui / wgpu GLES。


## 2026-10-10 Help 自动输入法修复

基于官方 v0.5.0 菜单，仅调整 Help：完成点击后再打开菜单和聚焦搜索框，避免按钮松手令搜索框失焦；搜索框内部点击保持菜单，菜单外点击与命令执行仍正常关闭。

模拟器 2880×1920 验证：单次点击 Help 自动弹出键盘并持续保持；输入法拼音候选“你好”写入搜索框；点击搜索框不关闭菜单；点击外部关闭后再次单击 Help，仍自动弹键盘。整个过程应用 PID 保持不变，没有应用闪退。[中文输入截图](docs/native/native-help-auto-ime.jpeg)。

菜单 12 项测试（包含点击、键盘缩放、搜索输入的真实 UI 回归）、原生桥接 11 项测试、ArkTS 输入法测试、Clippy 与分层检查通过；ARM64/x86_64 原生库和 HAP 构建、原生包校验通过。已安装模拟器，真实 Pad 尚未验证本次修复。

## 2026-10-10 原生手指笔头与双指导航修复

原生 XComponent 触摸不再只保留一个触点。C++ 每次转发全部手指触点的 ID 和坐标，Rust 根据快照生成 egui Touch Start/Move/End/Cancel，并只为首个手指合成兼容指针。第二根手指接管画布导航，缩放锚点为两指中心；剩余手指不会重新开始工具操作。手写笔接管时取消手指触点，抑制掌触至全部抬起。屏幕触摸期间阻止兼容鼠标事件覆盖手指来源，画布隐藏手指笔头，鼠标与手写笔仍显示笔头。

触控板 Axis 事件现在转发指针位置、平移量和累计捏合比例，Rust 将累计比例转换为 egui 每次 Zoom 比例，并在手势结束/取消及焦点丢失时清理状态。平移和位置统一转换为 egui points。

模拟器发现 XComponent 的 MOVE 触点 `isPressed` 为 false，直接用它过滤会把滑动变成起点；最终代码以触点阶段判断生命周期，回归测试包含这个真实序列。诊断日志已删除。

验证：

- 原生适配器 16/16 单元/集成测试通过，包括真实画布双指缩放/平移且文档 revision 不变、手指无笔头且鼠标/笔仍有笔头、抬起/取消、笔接管与掌触隔离。
- `python3 scripts/test_native_touch.py` 通过，执行实际 C++ 回调的触点快照、MOVE isPressed=false、双指、释放、笔优先级、轴事件捏合/平移与位置序列。
- ui-egui 默认 eframe-host 配置：883 passed，3 ignored；原生 ARM64 生产目标及 ui-egui all-targets 严格 Clippy 通过。
- 离线 metadata、28 crates 分层检查、cargo xtask wasm、git diff --check 通过。
- 最终 ARM64/x86_64 release 库、双架构 HAP 与 verify_native.py 通过；只安装到模拟器 127.0.0.1:5555，未部署已连接的真实 Pad。
- 模拟器 2880×1920 实测新建、手指点击/拖动画笔、笔画提交：连续拖动形成完整直线，抬起后无残留笔头。本次仅使用新建的合成测试文档，没有修改用户文档。
- 原生 `cargo clippy --tests -- -D warnings` 被既有测试中的 unwrap/panic 与 crate 级 deny 冲突阻断；生产目标严格检查通过。ui-egui 测试必须使用默认 eframe-host，关闭该特性会使其既有 eframe 测试无法编译。

真实 Pad 的屏幕双指、硬件触控板捏合/滚动、手写笔压力/倾斜与掌触手感尚需设备验收；上述双指/轴事件验证由 C++ 回调测试及真实画布集成测试完成，不能替代硬件验收。笔悬停仍属于既有未实现范围。

固定分辨率 MOVE 生命周期诊断对照：第一次实现错误过滤 MOVE，只留下起点 [诊断时截图](docs/native/native-touch-move-before.jpeg)；最终阶段过滤生成完整笔画 [修复后截图](docs/native/native-touch-move-after.jpeg)。两张均为 2880×1920，前图是本次调试的中间版本，不是任务前原始分支截图。

## 2026-10-10 手写笔入口时机与原生鼠标光标

手写笔选项仅在平台首次报告连接后显示；手指/笔尖接触本身不会令入口出现。首次连接后在本次应用运行中保持显示，断连仅更新连接状态。共享真实 UI 测试覆盖初始隐藏、笔接触不显示、连接显示及断连保留。

原生工作线程将 egui 最终光标通知 ArkUI：`None` 隐藏系统箭头，由画布绘制笔头或工具光标；十字、文本、手形、缩放和调整大小等使用鸿蒙对应样式。修正了合并更早的逻辑帧输出时覆盖 UI 光标的问题。窗口失焦、页面销毁、切后台恢复指针可见性，前台/重新附着清除缓存以重新同步。

验证：原生 18 项测试通过；ui-egui 882 项通过、3 项忽略，唯一在沙箱中因 GPU adapter 不可用失败的快捷键测试已在可访问 GPU 的环境单独重跑通过（共 883 项）。ArkTS 光标、host-mode、system-bars 脚本通过。ARM64 生产目标严格 Clippy 通过；ui-egui all-targets 在允许既有 `Services.default_save` 的 `type_complexity` 告警后通过，其余告警仍按错误处理。离线 metadata、28 crates 分层、wasm 检查及 git diff --check 通过。

ARM64/x86_64 release 原生库、HAP 构建与包校验通过；模拟器 127.0.0.1:5555 安装/启动成功，未部署真实 Pad。签名产物最初在 ZIP EOCD 后多出一个空字节，导致系统 ERR_INSTALL_PARSE_UNEXPECTED；安装副本移除该尾部字节后成功，标准 signed.hap 已替换为该已验签安装的副本。未修改签名材料或应用源代码以规避检查。

2880×1920 欢迎页对照：[修改前](docs/native/native-stylus-before.jpeg)、[修改后](docs/native/native-stylus-after.jpeg)。无手写笔模拟器的入口已隐藏，日志确认 `cursor: Default` 到达 ArkTS 且未出现光标 SDK 异常。模拟器检查不能证明硬件鼠标隐藏/样式切换或真实手写笔插拔，仍需 Pad 验收。

## 2026-10-10 手写笔入口移至菜单栏

手写笔入口移到顶部菜单栏右侧、Discord 左侧，文字采用与菜单项相同的 Button 字体及 text_dim 颜色；移除工具选项栏中的入口、分隔线和宽度占位。标题栏为手写笔预留空间后再决定可选 Discord/主题/搜索按钮。连接检测与断连保留规则不变。

5 项相关真实 UI/设置测试通过，包括入口位置、没有重复入口、连接显示及设置持久化。截图渲染在可用 GPU 环境重跑通过，已目视确认位置、字号和颜色。ui-egui all-targets 严格 Clippy、离线 metadata、git diff --check、ARM64/x86_64 原生库、HAP 构建及原生包校验通过。本次未安装设备；外观通过离屏真实 UI 渲染验证。

菜单栏细调：连接圆点左移 3 points，手写笔与 Discord 控件间距由 6 增至 16 points；宽度预留同步调整。5 项相关测试、带连接圆点的真实 UI 渲染、严格 Clippy、双架构 HAP 构建及包校验通过；本次未部署设备。

## 2026-10-10 双指画布导航起点限制

按画布记录触点 Start 的落点，只允许整个触摸序列的触点从无遮挡画布开始。侧栏、工具栏、首选项及浮窗上的起点拒绝导航，随后移入画布仍拒绝；全部抬起/取消后才重新判断。允许画布开始的手势移出边界并继续，平移与缩放使用同一资格判断。未更改鼠标滚轮/触控板的悬停规则。

验证：touch_nav 5 项通过（新增跨边界、混合起点、取消/重新开始回归）；原生 24 项全部通过，包含真实编辑器侧栏/首选项双指移入画布仍不改变 zoom/center，及有效画布双指正常导航且不编辑文档。UI 全量 888 passed / 3 ignored / 1 GPU adapter 不可访问；该项沙箱外单独复测通过。UI all-target Clippy、offline metadata、28 crates 分层、配置工具链的 wasm 检查通过。ARM64/x86_64 release 库、HAP 构建及 verify_native.py 通过。

API 20 模拟器使用独立 touchorigintest 包完成启动/新建文档/菜单烟测，截图为 2880×1920：[画布](docs/native/touch-origin-canvas.jpeg)。测试包随后移除，没有替换或重启已有含未保存文档的应用，也没有向真实 Pad 部署。模拟器 uitest CLI 只支持单指注入；双指行为由实际原生触摸快照→egui→真实编辑器集成测试验证，硬件多指手感尚未验收。

## 2026-10-10 HarmonyOS 项目入口与关于页面精简

原生适配层启用共享 UI 的 `harmonyos-ui` feature。标题栏、启动页、帮助菜单隐藏 Discord、PhotoCraft/ArtCraft 官网、GitHub 及问题反馈入口；命令搜索使用相同菜单数据。“关于”移除链接与贡献者/模型页签，保留简介、版本和技术栈，窗口宽度随内容收窄。旧 dialog `tab=contributors/models` 状态回退到精简简介。系统信息、保存选择器及系统浏览器服务保持原有行为；默认上游桌面/Web 界面保留原入口。

验证：HarmonyOS feature 下 UI 全量 952 passed / 3 ignored（含 890 项单元测试），原生 24 项通过；默认 feature 下 3 项链接/菜单测试通过。UI all-targets 严格 Clippy、离线 metadata、28 crates 分层检查通过。初次 wasm 检查使用 Homebrew Rust，因缺少目标标准库失败；加载 `scripts/dev.local.env` 配置工具链后完整 wasm 检查通过。ARM64/x86_64 release 库、HAP 构建及 verify_native.py 通过。

仅在 API 20 模拟器 127.0.0.1:5555 安装并启动新版，部署前处于无文档状态，未部署真实 Pad。目视核对启动页、帮助菜单及关于窗口，截图均为 2880×1920：[启动页修改前](docs/native/project-links-home-before.jpeg)、[启动页修改后](docs/native/project-links-home-after.jpeg)、[关于修改前](docs/native/project-links-about-before.jpeg)、[关于修改后](docs/native/project-links-about-after.jpeg)。

## 2026-10-10 恢复 fork GitHub 与问题反馈入口

鸿蒙帮助菜单恢复 `help.github` 与 `help.reportIssue`，分别指向 `https://github.com/MXJinjing/photocraft-hmos` 和该 fork 的 `/issues`。命令搜索沿用菜单数据；默认上游构建仍指向 storytold/photocraft。Discord、官网、主页推广链接及关于窗口的链接/贡献者/模型页签继续隐藏。链接仍通过已有系统浏览器服务打开。

验证：鸿蒙 UI 链接/真实界面 4 项回归、默认上游链接 3 项回归通过；鸿蒙 UI 全量 952 passed / 3 ignored，当前工作区原生 26 项通过。UI all-targets 严格 Clippy、离线 metadata、28 crates 分层与配置工具链 wasm 检查通过。双架构 release 库、HAP 构建和包校验通过；首次构建遇到工作区并行保存接口修改时的暂时签名不一致，接口对齐后重试成功，未改动其他任务的代码。

在无文档状态的 API 20 模拟器 127.0.0.1:5555 安装/启动并目视核对恢复的菜单项，未部署真实 Pad；URL 路由由记录平台 open_url 服务的回归测试验证。2880×1920 帮助菜单对照：[恢复前](docs/native/fork-links-help-before.jpeg)、[恢复后](docs/native/fork-links-help-after.jpeg)。

## 2026-10-10 在 dev 合并 upstream v0.6.0，仅构建 ARM64

通过未 squash 的 subtree 合并导入 v0.6.0（0c72d95425dece90ef9a1cceb49e3315c96e22d5）。重新接入不依赖 eframe 的鸿蒙 host、系统字体、中文输入、同步平台文件服务、保存完成守卫、打印、fork 链接和精简关于页面；保留笔事件与手指导航规则，并接入上游新的笔压队列与旋转视图。修复合并后的重复画笔弹窗和丢失的鸿蒙翻译条目，补全上游新增语言中的平台选项。裁剪旋转被导航打断时恢复原角度，附回归测试。vendored wgpu-hal 30.0.1 补丁保留，两个 workspace 的 lockfile 已同步升级。

应用版本 0.6.0.1 / 60001。原生构建脚本、CMake ABI、Hvigor abiFilters、CI 与包校验统一为 aarch64 / arm64-v8a；额外 x86_64 原生库会被包校验拒绝。ARM64 构建配置及合成 HAP 回归测试通过。原有 ArkTS host-mode/system-bars 测试补齐已存在的 WindowChrome mock。

验证：默认 UI/engine/text 单元、集成及文档测试共 2,872 passed / 28 ignored；harmonyos-ui 单元测试 1,527 passed / 8 ignored，另新增裁剪恢复测试通过；最终原生适配器 26 项通过。共享三个 crate 以及鸿蒙 UI all-targets 严格 Clippy、ARM64 原生库严格 Clippy、offline metadata、分层、L0–L6 wasm 与 photocraft-web wasm 编译通过。全部 12 个 ArkTS 回归脚本及 4 个 Python/C++ 回归脚本通过。

ARM64 release Rust 库与本地 debug HAP 构建、verify_native.py 通过。API 20 ARM64 模拟器完成最终 signed HAP 安装/启动，目视确认欢迎页正常渲染；未部署真实 Pad。两张截图均为 2880×1920：[v0.5.0](docs/native/upstream-v050-home.jpeg)、[v0.6.0](docs/native/upstream-v060-home.jpeg)。文件服务/输入行为由平台脚本与真实编辑器集成测试覆盖，本轮模拟器只完成启动页烟测。
