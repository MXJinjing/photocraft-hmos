@CustomDialog
export struct LocalSaveDialog {
  controller: CustomDialogController;
  initialName: string = '';
  onConfirm: (name: string) => void = (_name: string): void => {};
  onCancel: () => void = (): void => {};
  @State private name: string = '';
  @State private error: string = '';

  aboutToAppear(): void { this.name = this.initialName; }

  private confirm(): void {
    let name = this.name.trim();
    if (name.length === 0 || name === '.' || name === '..' || /[\\/:*?"<>|\x00-\x1f]/.test(name)) {
      this.error = '请输入有效的文件名，不能包含路径或特殊字符';
      return;
    }
    if (!name.toLowerCase().endsWith('.psd')) { name += '.psd'; }
    this.controller.close();
    this.onConfirm(name);
  }

  build() {
    Column({ space: 16 }) {
      Text('保存 Photoshop 文件').fontSize(20).fontWeight(FontWeight.Medium)
      Text('保存到下载目录中的 PhotoCraft 文件夹。重名时自动添加数字。').fontSize(14)
      TextInput({ text: this.name, placeholder: '文件名.psd' })
        .onChange((value: string): void => { this.name = value; this.error = ''; })
        .onSubmit(() => this.confirm())
      if (this.error.length > 0) { Text(this.error).fontColor('#D94848').fontSize(14) }
      Row({ space: 16 }) {
        Button('取消').onClick(() => { this.controller.close(); this.onCancel(); })
        Button('保存').onClick(() => this.confirm())
      }.justifyContent(FlexAlign.End).width('100%')
    }.padding(24).width('100%')
  }
}
