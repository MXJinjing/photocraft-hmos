# PhotoCraft Pad（ArkWeb 原型）

本工程把 [PhotoCraft v0.3.0](https://github.com/storytold/photocraft/releases/tag/v0.3.0) 网页版封装为 HarmonyOS 6.0 / API 20 平板应用。已在 HarmonyOS 6.0(20) 平板模拟器完成启动、绘图、File → Save／Save As 保存 PSD、PNG 导出及从本地重新打开的基本测试。测试详情见 [TEST_REPORT.md](TEST_REPORT.md)，实施计划见 [PLAN.md](PLAN.md)。

## 构建和运行

使用 DevEco Studio 26.0.0.851 打开本目录，选择 `entry/default` 并构建 debug HAP。命令行可在 DevEco 工具环境中执行 `hvigorw assembleHap --mode module -p product=default -p buildMode=debug --no-daemon`。当前生成的 `entry/build/default/outputs/default/entry-default-unsigned.hap` 可安装于测试模拟器；连接实机前需在 DevEco Studio 配置自己的调试签名并重新构建。

网页资源位于 `entry/src/main/resources/rawfile`。ArkWeb 把应用内部的 `https://photocraft.invalid/index.html?webgl&cpu` 映射到打包资源，正常编辑不依赖联网。`webgl` 用于界面渲染，`cpu` 让 PhotoCraft 在网页端使用 CPU 文档画布路径，以避开当前模拟器的 WebGL 画布绘制异常。此选择可能影响大画布性能，需在实机评估。

## 文件操作

- “打开”调用系统 `DocumentViewPicker`，所选文件 URI 交给 ArkWeb。模拟器中已从系统“文档”重新打开导出的 PNG。
- 在 PhotoCraft 菜单点 File → Save 或 Save As 后，ArkWeb 收到网页生成的 PSD 下载，自动打开鸿蒙系统保存界面。选定目录并点“保存”即可写入本地，无需再点右上角导出按钮。模拟器中已保存并重新打开含两层和笔迹的 `Untitled-1.psd`（约 267 KB）；Save As 也已保存到另一个本地目录。
- File → Export 中的 PNG 等下载同样自动进入系统保存界面。模拟器中已保存 `Untitled-1.png`（约 18 KB）并重新打开。
- 如果取消系统保存界面，右上角会保留“点击重试”；文件不会因此写入目标目录。当前每次点 Save 都会再次选择保存位置，尚未记录目标文件以直接覆盖。网页状态栏会先显示 Saved，即使系统保存仍在进行或被取消；判断是否成功请以系统文件中出现作品为准。

## 资源来源与重建

`third_party/photocraft/manifest.json` 固定官方网页发布包、官方 v0.3.0 源码归档、适配后资源及校验值。`scripts/verify_assets.py` 可校验发布包、适配资源、HTML 引用、ArkWeb 映射和补丁哈希。原版静态资源保存在 `third_party/photocraft/release-web`，适配版在 `rawfile`。

上游静态包在模拟器上因 WebGL shader 上限与驱动行为无法直接启动。`third_party/photocraft/photocraft-source.patch` 包含设备上限兼容、网页端 CPU 文档合成和 WASM 错误日志；`wgpu-hal-uniform-block.patch` 修正该版本 wgpu-hal 30.0.1 对被驱动优化掉的 uniform block 解引用。重建时先校验官方源码归档和 wgpu-hal 30.0.1 crate 的 SHA-256，再分别应用补丁；在上游 Cargo 工作区增加指向已打补丁 wgpu-hal 源码的 `[patch.crates-io]` 路径覆盖，执行 `trunk build --release`。最后将生成的 HTML、JS、WASM 放入 `rawfile`，同步 `Index.ets` 中资源文件名和 manifest 哈希，并重新打包 HAP。上游双许可证见 `third_party/photocraft/`。

## 下一阶段

模拟器无法验证 HUAWEI Pencil 的压感、倾角、悬停、橡皮擦和手掌防误触；这些项目须在用户连接 HarmonyOS 6.0 Pad 后测试。JPEG 导入、多指缩放、偏好持久化及大画布性能也尚未完成验收。系统文件选择器内修改文件名、已有文件覆盖及中文文件名仍待补测。网页仍采用桌面布局。
