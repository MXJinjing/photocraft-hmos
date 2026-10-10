# PhotoCraft HarmonyOS 原生自绘实验

当前分支 `codex/hmos-native-selfdraw` 将 PhotoCraft v0.5.0 作为 Rust 原生库运行，通过 XComponent / NativeWindow 和 wgpu GLES/EGL 绘制原有 egui UI。鸿蒙入口统一使用原生 Rust。HAP 不包含网页或 WASM 快照。文档画布暂使用 CPU 合成。

```bash
scripts/dev.sh build   # 双架构 Rust 库 + 本地 debug HAP
scripts/dev.sh run     # 构建、安装到唯一连接的模拟器/设备并启动
scripts/dev.sh launch  # 重新启动已安装应用，进入原生模式
```

需要 HarmonyOS 6.0 / API 20 SDK、Rust OHOS 编译目标，以及本地调试签名。工具路径沿用被忽略的 `scripts/dev.local.env`，参考 `scripts/dev.local.env.example`。版本保持 `0.5.0.1`，本轮不发布 release。

源码仍位于 `upstream/photocraft/`；独立原生适配层在 `native/rust/` 与 `entry/src/main/cpp/`。修改 Rust 或 ArkTS/C++ 后须重建部署 HAP。开发流程统一使用原生构建与调试；上游桌面与 Web 源码随 subtree 历史保留，但不参与鸿蒙构建。

首轮范围是完整 UI 与基本编辑、PSD/PNG 打开保存闭环。Open Recent 的持久 URI 授权仍待补齐，重开文件请使用系统选择器。Save / Save As / 导出使用系统保存选择器，外链交给系统应用。中文输入法、系统剪贴板、完整手写笔能力、自动恢复和真机性能验收不在首轮范围。

详细架构与构建说明见 [native/README.md](native/README.md)，验证记录见 [NATIVE_TEST_REPORT.md](NATIVE_TEST_REPORT.md)。完整旧 ArkWeb 版本保留在 `main` 与 Git 历史中。

签名证书、私钥和密码只能保留在本地，不能提交；CI 继续手动触发并输出 unsigned HAP。


`launch` 和重新部署会关闭当前应用，请先保存文档。收到新的文件 Want 时，文件继续交给当前原生编辑器。
