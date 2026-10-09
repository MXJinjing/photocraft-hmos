// The window-stage callback lives in EntryAbility; the page owns the ArkWeb controller.
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
