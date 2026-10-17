@CustomDialog
export struct OpenSourceDialog {
  controller: CustomDialogController;
  onFiles: () => void = (): void => {};
  onGallery: () => void = (): void => {};
  onCancel: () => void = (): void => {};

  build() {
    Column({ space: 16 }) {
      Text('打开文件').fontSize(20).fontWeight(FontWeight.Medium)
      Text('请选择文件来源').fontSize(14)
      Button('从文件管理选取')
        .width('100%')
        .onClick(() => {
          this.controller.close();
          this.onFiles();
        })
      Button('从图库选取')
        .width('100%')
        .onClick(() => {
          this.controller.close();
          this.onGallery();
        })
      Button('取消')
        .width('100%')
        .onClick(() => {
          this.controller.close();
          this.onCancel();
        })
    }.padding(24).width('100%')
  }
}
