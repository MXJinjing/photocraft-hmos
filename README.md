# PhotoCraft HarmonyOS 原生自绘实验

当前开发分支 `dev` 将 PhotoCraft v0.6.0 作为 Rust 原生库运行，通过 XComponent / NativeWindow 和 wgpu GLES/EGL 绘制原有 egui UI。鸿蒙入口统一使用原生 Rust。HAP 不包含网页或 WASM 快照。文档画布暂使用 CPU 合成。

原生宿主会查询窗口广色域能力，并协商 Display P3 输出；系统不支持或拒绝时使用明确标记的 sRGB。PhotoCraft 的自动显示配置文件表示应用输出空间，鸿蒙负责最终的屏幕色彩映射，不是实测屏幕 ICC。画布按文档 ICC 转换，普通 egui 界面颜色从 sRGB 转到同一输出空间；窗口重建、尺寸变化和 EGL surface 恢复后重新声明色彩空间。此接入支持 SDR 广色域显示，预览仍为 8-bit，不包含 HDR 图片增益图或 HDR 屏幕输出。

```bash
scripts/dev.sh build   # ARM64 Rust 库 + 本地 debug HAP
scripts/dev.sh run     # 构建、安装到唯一连接的模拟器/设备并启动
scripts/dev.sh launch  # 重新启动已安装应用，进入原生模式
```

需要 HarmonyOS 6.0 / API 20 SDK、Rust OHOS 编译目标，以及本地调试签名。工具路径沿用被忽略的 `scripts/dev.local.env`，参考 `scripts/dev.local.env.example`。当前版本为 `0.6.0.1`（versionCode `60001`）。

鸿蒙构建启用 `photocraft-ui-egui/harmonyos-ui`：标题栏、启动页和帮助菜单隐藏 Discord 及项目官网入口；帮助菜单保留 GitHub 与问题反馈，分别指向 `MXJinjing/photocraft-hmos` fork 及其 Issues；“关于”仅保留简介、版本与技术栈，不显示链接、贡献者和模型页签。上游桌面/Web 默认界面不受影响。

源码仍位于 `upstream/photocraft/`；独立原生适配层在 `native/rust/` 与 `entry/src/main/cpp/`。修改 Rust 或 ArkTS/C++ 后须重建部署 HAP。开发流程统一使用原生构建与调试；上游桌面与 Web 源码随 subtree 历史保留，但不参与鸿蒙构建。

首轮范围是完整 UI 与基本编辑、PSD/PNG 打开保存闭环。Open Recent 的持久 URI 授权仍待补齐，重开文件请使用系统选择器。Save / Save As / 导出使用系统保存选择器，外链交给系统应用。中文输入法、系统剪贴板、完整手写笔能力、自动恢复和真机性能验收不在首轮范围。

详细架构与构建说明见 [native/README.md](native/README.md)，验证记录见 [NATIVE_TEST_REPORT.md](NATIVE_TEST_REPORT.md)。完整旧 ArkWeb 版本保留在 Git 历史中。

签名证书、私钥和密码只能保留在本地，不能提交；CI 继续手动触发并输出 unsigned HAP。


`launch` 和重新部署会关闭当前应用，请先保存文档。收到新的文件 Want 时，文件继续交给当前原生编辑器。

## 许可证

本项目与上游一致，采用 [MIT](LICENSE-MIT) 或 [Apache-2.0](LICENSE-APACHE) 双许可，可任选其一（`MIT OR Apache-2.0`）。贡献默认采用相同许可。

上游版权与归属声明见 [NOTICE](upstream/photocraft/NOTICE) 和 [ATTRIBUTION.md](upstream/photocraft/ATTRIBUTION.md)。第三方代码及资源保留各自许可证；ArtCraft 品牌资源另受 [品牌许可](upstream/photocraft/docs/brand/LICENSE-brand.txt) 约束。
