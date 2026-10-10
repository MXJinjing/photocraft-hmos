# PhotoCraft Pad 测试记录

此文件记录历史 ArkWeb 版本。当前原生分支的构建与验收以 `NATIVE_TEST_REPORT.md` 为准；历史 Web 命令和资源校验脚本已移除。

## 环境与产物（2026-10-08）

- 工具：DevEco Studio 26.0.0.851，内置 HarmonyOS SDK 26.0.0.105，Hvigor 6.26.8。
- 虚拟设备：`PhotoCraftPadAPI20`，HarmonyOS 6.0.0(20) 平板镜像，2880×1920 屏幕。
- 构建：ArkTS 编译和 debug HAP 打包成功；目标和兼容 API 均为 20。
- 产物：`entry/build/default/outputs/default/entry-default-unsigned.hap`，SHA-256 `11d14f2fd04140b1d4cd483eee54d1113b0d5766be3b971fa27a6b39587392d6`，约 24 MB。未配置签名；此产物已在模拟器安装并启动。
- 资源检查：`python3 scripts/verify_assets.py` 通过；HAP 包含 HTML、JS、WASM 和 ArkTS 字节码。

## 模拟器结果

| 检查项 | 结果 |
| --- | --- |
| 安装与启动 | 通过：HDC 安装成功，ArkWeb 显示 PhotoCraft 首页 |
| 新建文档 | 通过：默认 1920×1080 白色画布、背景图层显示正常 |
| 图层和绘制 | 通过：新增像素图层，模拟触控拖动留下可见线条 |
| PNG 导出 | 通过：先前版本已完成 Quick Export as PNG、系统保存及重新打开；新版共用同一下载保存回调，尚未单独复测 PNG |
| File → Save | 通过：无需点右上角按钮，系统保存界面自动出现；`Untitled-1.psd` 已写入“文档”，文件选择器显示约 267 KB |
| PSD 重新打开 | 通过：从系统“文档”打开 PSD，笔迹及背景、Layer 1 两层一致 |
| File → Save As | 通过：自动打开系统保存界面，并将同名 PSD 保存到“图片”；文件选择器可见第二份 PSD |
| 取消保存 | 关闭系统保存界面即放弃本次保存，不再显示右上角重试气泡 |
| 保存到系统文件 | 通过：系统保存界面将 `Untitled-1.png` 写入“文档”，文件选择器显示约 18 KB |
| PNG 重新打开 | 通过：经系统文件选择器打开，出现第二个文档标签，笔迹与导出前一致 |
| HUAWEI Pencil | 不适用：模拟器没有真实手写笔硬件及压力、倾角数据 |
| JPEG 导入、缩放、偏好持久化、大文件性能 | 尚未测试；进入实机阶段前仍需按计划补测 |

## 兼容处理

原版发布资源直接运行时，模拟器 WebGL 适配器只提供 7 个 inter-stage shader variables，上游要求值 15，导致启动失败。源码适配后又遇到 wgpu-hal 对已优化 uniform block 的解引用。解决这两项后首页可显示，但 GPU 文档画布在新建文档时出现 WebGL 错误，模拟器图形服务卡顿并曾截取到全黑画面。当前 HAP 使用 `?webgl&cpu`，让 PhotoCraft 不启用 GPU 文档画布；默认画布、绘制、PNG 和 PSD 往返测试均通过。此处仅证明模拟器基本路径可用，尚未证明真机性能或长时间稳定性。

### 去掉 `&cpu` 的实测（2026-10-08）

将离线和开发启动 URL 临时改为 `?webgl`，重新编译、签名并覆盖安装 HAP，在同一平板模拟器中启动离线包。首页正常，Help → System Info 显示 `Graphics adapter: Mali-G77`、`Backend: gl (integratedGpu)`、`Driver: WebGL 2.0 (OpenGL ES 3.0 Chromium)` 和 `Canvas renderer: GPU`。新建默认 1920×1080、白色背景的文档后，背景图层缩略图为白色，主画布却显示为黑色。GPU 初始化成功，但该模拟器中的文档画布绘制不正确。测试后恢复 `?webgl&cpu` 并重新构建可用的 HAP；实机 GPU 路径仍待验证。

## 真机待验证

连接 HarmonyOS 6.0 Pad 后重新配置调试签名、安装并复测：HUAWEI Pencil 点位、笔压、倾角、悬停、橡皮擦和手掌防误触；多指缩放；JPEG 导入、PSD 在实机上的保存和打开；文件改名、覆盖与中文文件名；重启后的偏好；不同画布尺寸及连续绘制性能。记录设备型号、系统版本、ArkWeb 版本和失败步骤。

## 当前保存流程的边界

每次 File → Save 都会重新打开系统保存位置选择器，尚未做到第一次保存后原位覆盖。网页端会在系统界面完成前显示“Saved”；若用户取消，本次保存被丢弃，不再保留右上角重试入口。模拟器的小艺输入法首次启用要求接受独立条款，因此未完成系统选择器内改名测试。HUAWEI Pencil 仍待实机。

## 源码仓库与实时调试（2026-10-08）

- 在鸿蒙工程目录初始化 Git 仓库，配置 `upstream` 远程，获取 PhotoCraft v0.3.0 标签与主分支，再以未压缩历史的 subtree 导入源码。已验证 `v0.3.0` 是本仓库当前历史的祖先，上游提交记录可通过 Git 查看。
- 适配源码与打补丁的 wgpu-hal 30.0.1 均已纳入仓库。离线打包版资源仍保留，`python3 scripts/verify_assets.py` 通过。
- Rust 工作区离线依赖解析成功；Trunk 0.21.14 从仓库源码完成 WASM 构建；HarmonyOS 6.0(20) debug HAP 编译成功。模拟器使用 `hdc rport` 打通开发服务，ArkWeb 显示源码构建的 PhotoCraft 首页。
- `scripts/dev.sh run` 端到端复测通过：自动构建 HAP、覆盖安装、启动 Trunk、建立本地连接并打开应用。
- 临时修改网页 HTML 后，Trunk 自动重建且模拟器页面自行刷新，出现 `LIVE RELOAD TEST`；还原后自动消失。临时修改 Rust 界面文字后，Trunk 自动重编译 WASM，模拟器自行刷新并显示 `live Rust change`。测试用文字均已从源码还原。
- 测试 HAP 的签名与模拟器已安装版本不一致，保留数据卸载后仍无法重装；最终完整卸载才成功安装，因此模拟器应用私有数据在本轮测试中被清理。以后更换签名前应备份应用私有数据。
- Help 与 Discord 外链已由用户确认进入系统浏览器；此前离线 ArkWeb 的 Blocked 页面不再出现。

## 右键菜单与鼠标／触控板滚动修复（2026-10-09）

- 根因：手指防误触进入触控导航分支时，右键菜单的显示和关闭逻辑被跳过，菜单只是在按下期间隐藏，抬起后再次显示。菜单现独立于画布导航处理，手指仍可点击菜单项。
- 菜单接管整个关闭接触：手指或手写笔点击菜单外关闭后，继续按住、移动以及抬起均不绘制；下一次笔接触恢复绘制。覆盖画笔、选区工具和图层菜单。
- 滚动与兼容指针事件混合时，旧逻辑会同时平移画布和启动笔画。现在鼠标滚轮／触控板导航接管相关接触，直至抬起；四个方向滚动不产生横竖线。真实多指触屏缩放仍保留中心点平移。
- 新增四项输入回归测试，涵盖菜单关闭后长时间按住及抬起、手指点击菜单内容、笔关闭菜单后继续拖动与下一次绘制，以及四方向滚动混合指针／触摸事件。两项关闭缺陷与滚动绘制缺陷均在修复前复现。
- 验证：`cargo test --offline -p photocraft-ui-egui` 共 934 项通过、3 项忽略；全目标 Clippy（`-D warnings`）、Rust 格式检查、离线依赖解析、28 crate 层级检查和 `cargo xtask wasm` 通过。离屏截图确认画笔、选区和图层菜单显示，手指按下／抬起后菜单保持关闭。
- 重新执行 `scripts/package_offline.sh` 成功：Trunk release 构建、离线资源哈希校验与 HAP 编译／签名通过。产物 `entry/build/default/outputs/default/entry-default-signed.hap`，SHA-256 `ee92becea1a94fff9bad6901048cca68cda0a04c98a4e94b9a1f73f67cacd3c0`。`module.json5` 的 `launchMode` 已改为 SDK 支持的 `launchType`，配置校验通过。
- 本轮尚未在 HarmonyOS 模拟器或实机复测；HUAWEI Pencil、鼠标／触控板在 ArkWeb 中的实际事件仍待设备验证。

## 手写笔偏好同步与工具栏滚动（2026-10-09）

- Preferences › Tools 新增手写笔分组：屏蔽手指输入、双击笔身动作、长按笔身动作。与顶部手写笔菜单共享 `tools.blockFingerInput`、`tools.stylusDoubleTap`、`tools.stylusLongPress` 及选项文案；菜单立即修改已保存偏好，首选项通过 Apply／OK 应用草稿，取消保留已应用值。设置重启后保留，旧偏好文件缺少新字段时采用原有默认值。
- 工具栏保留原有单列／双列布局与宽度；高度不足时可纵向滚动，支持鼠标滚轮／触控板、手指拖动和滚动条。背景与分隔线留在固定面板中。拖动处理在首次触屏接触前即注册，避免首次手指滑动无效。
- 新增三项回归测试：偏好存取及非法值原子拒绝；真实菜单与首选项控件双向修改、Apply、取消、重启和笔身手势；Studio／Pro 两种主题的短工具栏通过滚轮及首次手指拖动滚动到 Zoom 工具并点击，滑动不误选工具、不平移画布也不绘制。
- 验证：engine 与 ui-egui 完整测试共 1748 项通过、18 项忽略；额外执行的命令异常参数检查 1 项通过。全目标 Clippy（`-D warnings`）、Rust 格式检查、离线依赖解析、28 crate 层级检查及 WASM 检查通过。
- 离屏截图已检查：Tools 中的手写笔分组排列正常，两种主题滚动后的底部工具可见。此轮尚未在 HarmonyOS 模拟器或实机进行运行验证。
- 新的 Trunk release 离线资源、哈希校验和 HAP 编译／签名成功。产物 `entry/build/default/outputs/default/entry-default-signed.hap`，SHA-256 `dedd5402b724c465e72f88ff837c9145d45016173da62a6426ef05c18a25f68e`。

## 统一手写笔接口与 HarmonyOS 适配器（2026-10-09）

- Rust 新增统一 `StylusInput` 入口，覆盖指针来源与笔采样、侧键状态、双击／长按动作和连接状态。浏览器 Pointer Events、原生采样入口及宿主桥接共用 `StylusFeed`；画布、菜单和首选项按通用动作工作。
- 宿主仅使用 `photocraft-stylus-input` 的 version 1 对象协议。旧 `photocraft-stylus`、`photocraft-stylus-connected` 事件和解码兼容已移除。校验版本、字段类型、动作及包大小；连接状态支持 WASM 启动时读取缓存，手势不会缓存或重放。
- `HarmonyStylusAdapter` 独立封装 Pen Kit／InputKit，负责生命周期、设备检测和 SDK 到通用动作的映射。时间统一为 epoch 毫秒，避免 SDK 与浏览器使用不同时间基准造成重复动作。支持局部 SDK 功能缺失；停止／重启后的旧异步查询不会更新状态。
- 共用 Rust／ArkTS 协议样例；适配器测试执行实际 ArkTS 工厂、注入脚本及适配器代码，验证协议一致性、单一事件发送、状态缓存、动作映射、幂等启停、旧查询隔离及部分功能不可用。
- 验证：ui-egui 完整测试 940 项通过、3 项忽略；web 测试 1 项通过；Node 适配器测试通过。全目标原生 Clippy（`-D warnings`）、格式检查、离线依赖解析、28 crate 层级检查、`cargo xtask wasm` 和 web WASM 编译通过。额外 WASM Clippy 在豁免已有 `dead_code` 警告后通过，其余警告视为错误。
- Trunk release、离线资源哈希检查及 ArkTS／HAP 编译和签名通过。产物 `entry/build/default/outputs/default/entry-default-signed.hap`，SHA-256 `148fb069f5243e4df8fc9a14a8bbe6f616eaf5df108324a00562192ee131be61`。
- 本轮未连接模拟器，未进行设备安装和运行验证；真实手写笔行为仍待设备验证。Android／iPadOS 原生适配器尚未实现，后续可接入相同接口。接口与接入说明见 `upstream/photocraft/docs/stylus-input.md`。
