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

后续阶段：中文输入法、系统剪贴板、完整手写笔能力、自动恢复、超过 256 MiB 文件与大图优化、真实 Pad 性能。当前仅基本单指绘画，完整多点手势尚未实现。
