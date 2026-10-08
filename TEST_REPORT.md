# PhotoCraft Pad 测试记录

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
| 取消保存与重试 | 通过：关闭系统保存界面后出现“保存已取消，点击重试”，点击可重新打开保存界面 |
| 保存到系统文件 | 通过：系统保存界面将 `Untitled-1.png` 写入“文档”，文件选择器显示约 18 KB |
| PNG 重新打开 | 通过：经系统文件选择器打开，出现第二个文档标签，笔迹与导出前一致 |
| HUAWEI Pencil | 不适用：模拟器没有真实手写笔硬件及压力、倾角数据 |
| JPEG 导入、缩放、偏好持久化、大文件性能 | 尚未测试；进入实机阶段前仍需按计划补测 |

## 兼容处理

原版发布资源直接运行时，模拟器 WebGL 适配器只提供 7 个 inter-stage shader variables，上游要求值 15，导致启动失败。源码适配后又遇到 wgpu-hal 对已优化 uniform block 的解引用。解决这两项后首页可显示，但 GPU 文档画布在新建文档时出现 WebGL 错误，模拟器图形服务卡顿并曾截取到全黑画面。当前 HAP 使用 `?webgl&cpu`，让 PhotoCraft 不启用 GPU 文档画布；默认画布、绘制、PNG 和 PSD 往返测试均通过。此处仅证明模拟器基本路径可用，尚未证明真机性能或长时间稳定性。

## 真机待验证

连接 HarmonyOS 6.0 Pad 后重新配置调试签名、安装并复测：HUAWEI Pencil 点位、笔压、倾角、悬停、橡皮擦和手掌防误触；多指缩放；JPEG 导入、PSD 在实机上的保存和打开；文件改名、覆盖与中文文件名；重启后的偏好；不同画布尺寸及连续绘制性能。记录设备型号、系统版本、ArkWeb 版本和失败步骤。

## 当前保存流程的边界

每次 File → Save 都会重新打开系统保存位置选择器，尚未做到第一次保存后原位覆盖。网页端会在系统界面完成前显示“Saved”；若用户取消，右上角重试入口会保留。模拟器的小艺输入法首次启用要求接受独立条款，因此未完成系统选择器内改名测试。HUAWEI Pencil 仍待实机。

## 源码仓库与实时调试（2026-10-08）

- 在鸿蒙工程目录初始化 Git 仓库，配置 `upstream` 远程，获取 PhotoCraft v0.3.0 标签与主分支，再以未压缩历史的 subtree 导入源码。已验证 `v0.3.0` 是本仓库当前历史的祖先，上游提交记录可通过 Git 查看。
- 适配源码与打补丁的 wgpu-hal 30.0.1 均已纳入仓库。离线打包版资源仍保留，`python3 scripts/verify_assets.py` 通过。
- Rust 工作区离线依赖解析成功；Trunk 0.21.14 从仓库源码完成 WASM 构建；HarmonyOS 6.0(20) debug HAP 编译成功。模拟器使用 `hdc rport` 打通开发服务，ArkWeb 显示源码构建的 PhotoCraft 首页。
- `scripts/dev.sh run` 端到端复测通过：自动构建 HAP、覆盖安装、启动 Trunk、建立本地连接并打开应用。
- 临时修改网页 HTML 后，Trunk 自动重建且模拟器页面自行刷新，出现 `LIVE RELOAD TEST`；还原后自动消失。临时修改 Rust 界面文字后，Trunk 自动重编译 WASM，模拟器自行刷新并显示 `live Rust change`。测试用文字均已从源码还原。
- 测试 HAP 的签名与模拟器已安装版本不一致，保留数据卸载后仍无法重装；最终完整卸载才成功安装，因此模拟器应用私有数据在本轮测试中被清理。以后更换签名前应备份应用私有数据。
- Help 与 Discord 外链已由用户确认进入系统浏览器；此前离线 ArkWeb 的 Blocked 页面不再出现。
