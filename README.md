# PhotoCraft（ArkWeb 原型）

本工程把 [PhotoCraft v0.3.0](https://github.com/storytold/photocraft/releases/tag/v0.3.0) 网页版封装为 HarmonyOS 6.0 / API 20 平板和 PC/2in1 应用。已在 HarmonyOS 6.0(20) 平板模拟器完成启动、绘图、File → Save／Save As 保存 PSD、PNG 导出及从本地重新打开的基本测试；PC/2in1 尚待设备验证。测试详情见 [TEST_REPORT.md](TEST_REPORT.md)，实施计划见 [PLAN.md](PLAN.md)。

## 构建和运行

使用 DevEco Studio 26.0.0.851 打开本目录，选择 `entry/default` 并构建 debug HAP。未签名包只能装模拟器；连接实机前需在 DevEco Studio 配置自己的调试签名。签名写在被 Git 忽略的根目录 `build-profile.json5`，新环境从 `build-profile.example.json5` 复制。日常开发和离线打包优先用下面的脚本，不必手写 hvigor 命令。

网页资源位于 `entry/src/main/resources/rawfile`。不带开发参数启动时，ArkWeb 把 `https://photocraft.invalid/index.html?webgl&cpu` 映射到这些打包资源，正常编辑不依赖联网。`cpu` 使用 CPU 画布，避免当前 WebGL 文档画布在绘画时又合成又上传。DevEco 直接 Run 只更新 HAP 外壳和图标，不会重新编译 `upstream/photocraft/` 里的 Rust 源码。

## 脚本

仓库自带两个 shell 脚本，都在 `scripts/`。它们按脚本自身位置找仓库根目录，当前工作目录不限。请直接执行，或用 `bash` 执行。不要用 `zsh 脚本路径` 去跑 `scripts/dev.sh`：那个文件使用 bash 的 `BASH_SOURCE`。`scripts/package_offline.sh` 则 bash 和 zsh 都可以。

两个脚本都会读取 Git 忽略的 `scripts/dev.local.env`。新机器若 Trunk、hdc 或 hvigor 不在默认位置，复制 `scripts/dev.local.env.example` 为 `scripts/dev.local.env` 并填写路径。可设置的变量：

- `PHOTOCRAFT_TRUNK`：Trunk 0.21.14 可执行文件。还需要该 Trunk 所用 Rust 的 `wasm32-unknown-unknown` 目标。
- `PHOTOCRAFT_HDC`：默认 `/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/hdc`。
- `PHOTOCRAFT_HVIGOR`：默认 `/Applications/DevEco-Studio.app/Contents/tools/hvigor/bin/hvigorw`。

运行前用 `hdc list targets` 确认只连着要安装的那一台设备。模拟器和实机同时在线时，脚本不会选择设备。

### `scripts/dev.sh`

用当前源码实时调试。应用通过 `hdc rport` 访问电脑上的 Trunk，地址是 `http://127.0.0.1:8765/index.html?webgl&cpu`。终端和数据线都要保持连接。修改 `upstream/photocraft/` 后 Trunk 自动重建并刷新，不必重装 HAP。修改 `entry/` 里的 ArkTS 后要重新执行 `run`。第一次 Rust 构建可能要数分钟。

```bash
scripts/dev.sh run
```

编译并安装已签名的 debug HAP，启动 Trunk，映射平板的 8765 端口，再以 `photocraft.dev` 开发参数打开应用。没有已签名 HAP 时会失败。

```bash
scripts/dev.sh serve
```

只在本机启动 Trunk，不安装、不启动应用。调试 HAP 已经装好时，在另一个终端执行：

```bash
scripts/dev.sh launch
```

它补上端口映射，并强制以开发模式重新打开应用。不带 `photocraft.dev` 的普通启动，包括 DevEco 的 Run 和桌面图标，继续使用 `rawfile` 里的离线包。

### `scripts/package_offline.sh`

把当前源码打成可离线运行的完整版本。它编译 release 网页包，替换 `entry/src/main/resources/rawfile`，更新 `Index.ets` 里的 JS/WASM 文件名和 `third_party/photocraft/manifest.json` 的哈希，再运行 `scripts/verify_assets.py`。默认还会打出已签名的 debug HAP。磁盘上未提交的鸿蒙源码改动会一起编进去。第一次编译可能要十几分钟。

```bash
scripts/package_offline.sh
```

编译、替换资源、校验并打包 HAP。产物是 `entry/build/default/outputs/default/entry-default-signed.hap`。之后从平板桌面打开，不要带 `photocraft.dev`。

```bash
scripts/package_offline.sh --assets-only
```

只替换离线资源并校验，不打 HAP。然后可以在 DevEco 里点 Run。

```bash
scripts/package_offline.sh --skip-build
```

跳过 Trunk，使用已有的 `upstream/photocraft/dist/web`。目录里必须已经有一次成功的 release 构建。

```bash
scripts/package_offline.sh --install
```

打包后用 hdc 安装到当前设备。安装前关掉模拟器。若平板上已有另一套签名的同名应用，需要先卸载。

## 源码

完整 PhotoCraft Rust 源码位于 `upstream/photocraft/`，已通过未压缩历史的 Git subtree 从 v0.3.0 更新到 v0.5.0。`git remote -v` 可看到当前目录的 `upstream` 远程，`git log v0.5.0` 可查看原版提交记录。Git 远程配置不会随克隆传播；在新克隆中执行 `git remote add upstream https://github.com/storytold/photocraft.git` 即可继续跟踪。鸿蒙适配改动直接写在这份源码中；`third_party/wgpu-hal-30.0.1/` 包含已打补丁的依赖源码。

## 文件操作

- “打开”调用系统 `DocumentViewPicker`，所选文件 URI 交给 ArkWeb。模拟器中已从系统“文档”重新打开导出的 PNG。
- 在 PhotoCraft 菜单点 File → Save 或 Save As 后，ArkWeb 收到网页生成的 PSD 下载，自动打开鸿蒙系统保存界面。选定目录并点“保存”即可写入本地。模拟器中已保存并重新打开含两层和笔迹的 `Untitled-1.psd`（约 267 KB）；Save As 也已保存到另一个本地目录。
- File → Export 中的 PNG 等下载同样自动进入系统保存界面。模拟器中已保存 `Untitled-1.png`（约 18 KB）并重新打开。
- Help、Discord 等外链交给系统默认浏览器打开，避免在离线 ArkWeb 页面中显示 Blocked。
- 界面语言跟随系统：简体中文、繁体中文、日语等已支持语言会在 Preferences › Interface › Language 为 Auto 时自动选用；也可在首选项里手动指定。
- File → 退出会关闭应用。系统返回键发给网页 Esc（取消工具、关闭对话框），不再直接回到桌面。
- 取消系统保存界面即放弃本次保存，文件不会写入目标目录，界面也不再出现重试提示。当前每次点 Save 都会再次选择保存位置，尚未记录目标文件以直接覆盖。网页状态栏会先显示 Saved，即使系统保存仍在进行或被取消；判断是否成功请以系统文件中出现作品为准。

## 手写笔与工具栏

- 手写笔输入、笔身动作和连接状态使用[统一接口](upstream/photocraft/docs/stylus-input.md)。HarmonyOS 的 Pen Kit／InputKit 封装在独立适配器中；其他平台可接入相同协议。当前未实现 Android／iPadOS 原生适配器，仅保留新版手写笔桥接事件。
- 顶部“手写笔”菜单与 Preferences › Tools › 手写笔共用设置：屏蔽手指输入、双击笔身动作和长按笔身动作。菜单修改立即生效；首选项中点 Apply 或 OK 后生效，设置会保留到下次启动。
- 工具栏高度不足时自动使用双列；仍放不下的内容可通过滚轮、触控板、拖动或滚动条纵向滚动。

## 资源来源与重建

`third_party/photocraft/manifest.json` 固定官方网页发布包、官方 v0.3.0 源码归档、适配后资源及校验值。`scripts/verify_assets.py` 可校验发布包、适配资源、HTML 引用、ArkWeb 映射和补丁哈希。原版静态资源保存在 `third_party/photocraft/release-web`，当前离线适配版在 `rawfile`；开发时以 `upstream/photocraft/` 源码为准。

上游静态包在模拟器上因 WebGL shader 上限与驱动行为无法直接启动。`third_party/photocraft/photocraft-source.patch` 记录设备上限兼容、网页端 CPU 文档合成和 WASM 错误日志；这些修改已应用到仓库源码。`wgpu-hal-uniform-block.patch` 记录依赖修复，其已打补丁源码存于 `third_party/wgpu-hal-30.0.1/`，Cargo 路径覆盖已配置。开发模式直接编译这份源码。离线包更新见上面的 `scripts/package_offline.sh`。上游双许可证见 `third_party/photocraft/`。

## 下一阶段

模拟器无法验证 HUAWEI Pencil 的压感、倾角、悬停、橡皮擦和手掌防误触；这些项目须在用户连接 HarmonyOS 6.0 Pad 后测试。JPEG 导入、多指缩放、偏好持久化及大画布性能也尚未完成验收。系统文件选择器内修改文件名、已有文件覆盖及中文文件名仍待补测。网页仍采用桌面布局。
