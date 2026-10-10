// The window-stage callback lives in EntryAbility; the selected editor guards unsaved work.
let requestPageClose: (() => void) | null = null;
let confirmedClose: boolean = false;

export function resetCloseGuard(): void {
  confirmedClose = false;
  requestPageClose = null;
}

export function setCloseRequest(handler: (() => void) | null): void {
  requestPageClose = handler;
}

export function requestWindowClose(): boolean {
  // true 拦截系统关闭，交给 Rust 检查未保存文档；确认后返回 false 放行后续关闭。
  if (confirmedClose || requestPageClose === null) {
    return false;
  }
  requestPageClose();
  return true;
}

export function allowConfirmedClose(): void {
  confirmedClose = true;
}

export function restoreCloseGuard(): void {
  confirmedClose = false;
}
