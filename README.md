# PhotoCraft Pad（ArkWeb 原型）

本工程把 [PhotoCraft v0.3.0](https://github.com/storytold/photocraft/releases/tag/v0.3.0) 网页版封装为 HarmonyOS 6.0 / API 20 平板应用。已在 HarmonyOS 6.0(20) 平板模拟器完成启动、绘图、File → Save／Save As 保存 PSD、PNG 导出及从本地重新打开的基本测试。测试详情见 [TEST_REPORT.md](TEST_REPORT.md)，实施计划见 [PLAN.md](PLAN.md)。

## 构建和运行

使用 DevEco Studio 26.0.0.851 打开本目录，选择 `entry/default` 并构建 debug HAP。命令行可在 DevEco 工具环境中执行 `hvigorw assembleHap --mode module -p product=default -p buildMode=debug --no-daemon`。当前生成的 `entry/build/default/outputs/default/entry-default-unsigned.hap` 可安装于测试模拟器；连接实机前需在 DevEco Studio 配置自己的调试签名并重新构建。

网页资源位于 `entry/src/main/resources/rawfile`。ArkWeb 把应用内部的 `https://photocraft.invalid/index.html?webgl&cpu` 映射到打包资源，正常编辑不依赖联网。`webgl` 用于界面渲染，`cpu` 让 PhotoCraft 在网页端使用 CPU 文档画布路径，以避开当前模拟器的 WebGL 画布绘制异常。此选择可能影响大画布性能，需在实机评估。

## 源码与实时调试

完整 PhotoCraft v0.3.0 Rust 源码位于 `upstream/photocraft/`。它通过未压缩历史的 Git subtree 导入，`git remote -v` 可看到当前目录的 `upstream` 远程，`git log v0.3.0` 可查看原版提交记录。Git 远程配置不会随克隆传播；在新克隆中执行 `git remote add upstream https://github.com/storytold/photocraft.git` 即可继续跟踪。鸿蒙适配改动直接写在这份源码中；`third_party/wgpu-hal-30.0.1/` 包含已打补丁的依赖源码。根目录 `build-profile.json5` 是本机签名配置，不纳入 Git；新环境可从 `build-profile.example.json5` 复制后在 DevEco Studio 配置签名。

安装 Rust wasm32-unknown-unknown 目标和 Trunk 0.21.14 后，连接 HarmonyOS 6.0 Pad 模拟器，执行 `scripts/dev.sh run`。若 Trunk 不在 PATH，可设置 `PHOTOCRAFT_TRUNK=/path/to/trunk`。脚本会编译并安装调试 HAP、启动源码服务、映射设备端 8765 端口，然后以开发模式启动应用。保持该终端运行；修改 `upstream/photocraft/crates/` 或网页版源码后，Trunk 自动重建 WASM，ArkWeb 自动刷新，无须复制编译文件或重装 HAP。第一次 Rust 构建可能需要数分钟。修改 ArkTS 包装代码后仍需重建 HAP。

如果调试 HAP 已安装，也可分别运行 `scripts/dev.sh serve` 和 `scripts/dev.sh launch`。开发页面通过设备本地 `http://127.0.0.1:8765/index.html?webgl&cpu` 获取；不带开发参数的普通启动继续使用离线打包资源。脚本默认使用 macOS 的 `/Applications/DevEco-Studio.app`，其他安装位置可通过 `PHOTOCRAFT_HDC` 和 `PHOTOCRAFT_HVIGOR` 指定。

## 文件操作

- “打开”调用系统 `DocumentViewPicker`，所选文件 URI 交给 ArkWeb。模拟器中已从系统“文档”重新打开导出的 PNG。
- 在 PhotoCraft 菜单点 File → Save 或 Save As 后，ArkWeb 收到网页生成的 PSD 下载，自动打开鸿蒙系统保存界面。选定目录并点“保存”即可写入本地，无需再点右上角导出按钮。模拟器中已保存并重新打开含两层和笔迹的 `Untitled-1.psd`（约 267 KB）；Save As 也已保存到另一个本地目录。
- File → Export 中的 PNG 等下载同样自动进入系统保存界面。模拟器中已保存 `Untitled-1.png`（约 18 KB）并重新打开。
- Help、Discord 等外链交给系统默认浏览器打开，避免在离线 ArkWeb 页面中显示 Blocked。
- 如果取消系统保存界面，右上角会保留“点击重试”；文件不会因此写入目标目录。当前每次点 Save 都会再次选择保存位置，尚未记录目标文件以直接覆盖。网页状态栏会先显示 Saved，即使系统保存仍在进行或被取消；判断是否成功请以系统文件中出现作品为准。

## 资源来源与重建

`third_party/photocraft/manifest.json` 固定官方网页发布包、官方 v0.3.0 源码归档、适配后资源及校验值。`scripts/verify_assets.py` 可校验发布包、适配资源、HTML 引用、ArkWeb 映射和补丁哈希。原版静态资源保存在 `third_party/photocraft/release-web`，当前离线适配版在 `rawfile`；开发时以 `upstream/photocraft/` 源码为准。

上游静态包在模拟器上因 WebGL shader 上限与驱动行为无法直接启动。`third_party/photocraft/photocraft-source.patch` 记录设备上限兼容、网页端 CPU 文档合成和 WASM 错误日志；这些修改已应用到仓库源码。`wgpu-hal-uniform-block.patch` 记录依赖修复，其已打补丁源码存于 `third_party/wgpu-hal-30.0.1/`，Cargo 路径覆盖已配置。开发模式直接编译这份源码。制作新的离线 HAP 时，在 `upstream/photocraft/apps/photocraft-web` 执行 `trunk build --release`，将 `upstream/photocraft/dist/web` 生成的 HTML、JS、WASM 同步到 `rawfile`，更新 `Index.ets` 资源名和 manifest 哈希，并重新打包 HAP。上游双许可证见 `third_party/photocraft/`。

## 下一阶段

模拟器无法验证 HUAWEI Pencil 的压感、倾角、悬停、橡皮擦和手掌防误触；这些项目须在用户连接 HarmonyOS 6.0 Pad 后测试。JPEG 导入、多指缩放、偏好持久化及大画布性能也尚未完成验收。系统文件选择器内修改文件名、已有文件覆盖及中文文件名仍待补测。网页仍采用桌面布局。
