# PhotoCraft HarmonyOS 6 Pad：ArkWeb 首版计划

## 目标与范围

在 HarmonyOS 6.0（API 20）平板上安装一个 HAP，离线运行 PhotoCraft v0.3.0 的现有网页版。首版保留网页界面，不移植 Rust 原生引擎或重做 ArkUI 编辑器。

首版“可用”的验收标准：能够启动并创建文档、用手指/手写笔绘图、通过系统文件选择器打开 PNG/JPEG/PSD、导出 PNG/PSD，并在重启后保留应用偏好。笔压、倾角及大文件性能需在真实平板上验证。

## 实施顺序

1. **固定网页版本**：使用上游 v0.3.0 发布的静态网页版包；记录来源、版本和校验值。优先使用发布包，避免为封装工程引入整套 Rust 编译链。
2. **创建 HarmonyOS 工程**：使用 Stage 模型、ArkTS 和 ArkWeb，目标为 tablet，兼容 SDK 设为 HarmonyOS 6.0 / API 20。网页资源放入 `entry/src/main/resources/rawfile`。
3. **解决资源加载**：以应用专用虚拟 HTTPS 源加载首页，利用 ArkWeb 的资源拦截接口只返回打包的 HTML、JavaScript、WebAssembly 等资源，并设置正确 MIME 类型；对未知路径返回 404，保证离线运行和同源加载。
4. **接通文件操作**：先验证网页版现有的文件选择与下载行为；若 ArkWeb 不完整支持，使用 ArkTS 的文件选择器和保存接口，通过受控 JS 桥接传递文件内容。避免仅有能启动但无法打开或保存作品的外壳。
5. **平板输入和显示**：先检查触摸、缩放、横屏与键盘；再在真机上验证 M-Pencil 的点位、压感、倾角和误触处理。需要时由 ArkTS 补充笔事件桥接。
6. **构建与验收**：生成 debug HAP，在 HarmonyOS 6.0 Pad 真机上按验收清单测试。记录设备型号、系统/API 版本、ArkWeb/WebGL2 能力、启动时间、内存及失败项。

## 关键决策与风险

- 使用 `https://photocraft.invalid/` 作为仅供应用内部使用的虚拟源；所有请求由本地资源拦截处理，不依赖线上站点。
- 上游网页需要 WebAssembly 和 WebGPU 或 WebGL2。优先验证 WebGL2 路径；在“安全防护模式”禁用 WebAssembly/WebGL2 的设备上，此方案无法运行。
- 上游网页版的保存方式是浏览器下载，ArkWeb 的下载行为和本地文件访问必须实测；这是达到“可用”的关键门槛。
- 现有网页界面是桌面布局，首版以横屏平板为主，触控可用性可能需要后续调整。
- 分发前核查 MIT/Apache-2.0 许可、NOTICE 和上游品牌资源的使用限制。

## 当前环境与状态

- 已完成 Stage 模型 ArkWeb 工程、离线资源映射、系统文件选择与保存回调。
- 已用 DevEco Studio 26.0.0.851 完成 ArkTS 编译与 debug HAP 打包，并在 HarmonyOS 6.0(20) 平板模拟器安装、启动。
- 上游网页资源经过 WebGL 设备限制与驱动兼容处理；模拟器使用 CPU 文档画布路径。补丁与资源哈希见 manifest 和 `TEST_REPORT.md`。
- 模拟器已通过首页、新建 1920×1080 画布、新增图层、模拟触控绘制、File → Save／Save As 自动打开系统保存界面、PSD 保存与重新打开，以及 PNG 导出往返测试。
- 当前 HAP 未签名。实机连接后配置调试签名，并验证 Pencil、JPEG、文件名/覆盖、缩放、偏好及性能；模拟器结果不能代替这些验收。

## 参考资料

- PhotoCraft v0.3.0: https://github.com/storytold/photocraft/releases/tag/v0.3.0
- HarmonyOS 6.0 / API 20: https://developer.huawei.com/consumer/en/doc/harmonyos-releases/overview-600
- ArkWeb 本地资源跨域和拦截: https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/web-page-loading
- 上游网页版部署要求: https://github.com/storytold/photocraft/blob/v0.3.0/packaging/web/README.md
