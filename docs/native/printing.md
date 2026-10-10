# PhotoCraft 鸿蒙打印与 PDF 保存

鸿蒙原生宿主通过 Basic Services Kit 的 `print.print([uri], UIAbilityContext)` 打开系统打印预览。需要在模块中声明 `ohos.permission.PRINT`。应用生成 PDF，打印机发现、驱动、份数、双面和实际打印任务由系统及打印扩展管理，不使用桌面 CUPS 的 `lp`。

参考：

- [OpenHarmony 打印 API](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/reference/apis-basic-services-kit/js-apis-print.md)
- [OpenHarmony 文件选择与 URI 授权](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/file-management/select-user-file.md)
- [华为鸿蒙打印机驱动安装说明](https://consumer.huawei.com/cn/support/content/zh-cn16049034/)

## 使用

文件 → 打印：先设置 PDF 的纸张、方向、缩放、颜色处理和印刷标记，点击打印进入系统打印面板，再选择打印机和实际输出参数。首次使用默认 A4；文档比纸张大时可勾选“缩放以适应纸张”。系统纸张与 PhotoCraft PDF 纸张应一致，避免额外缩放。

勾选“另存为 PDF”后，确认按钮变为“保存…”。系统 DocumentViewPicker 提供 PDF 格式，用户选择名称与位置后，宿主立即通过返回的 URI 打开文件并写入完整 PDF、fsync、关闭。Rust 仅收到一次性 `save-as/<id>/<name>` 令牌，不把令牌或 URI 转为普通路径。取消或写入失败不会报告成功，不需要申请用户整个文件目录的权限。普通保存/另存为的格式列表保留原有九种格式，只有 PDF 保存请求限定 PDF。

## 边界

`Session.print_service` 接收 PDF 字节与打印元数据，平台预览打开后返回 `previewOpened=true`、`sent=false`。这表示预览已打开，不表示纸张已经打印成功。宿主监听成功、失败、取消和阻塞事件；PDF 在任务终结后删除，阻塞时保留，进程退出后的残留文件交由应用缓存管理。无打印服务钩子的桌面端继续使用 CUPS；显式 PDF 输出在鸿蒙使用 `Session.print_save_service` 经授权写入。

鸿蒙打印机名称及多份打印参数应在系统面板设置。程序调用传入非空 CUPS 名称或大于一的份数会收到说明性错误。打印一份仍需用户在系统面板确认。打印机驱动安装、网络发现和实体输出须在具有相应打印机的真实 Pad 上验证。
