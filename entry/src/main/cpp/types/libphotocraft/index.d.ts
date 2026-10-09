export const init: (config: string, callback: (id: number, message: string, data: ArrayBuffer) => void) => void;
export const input: (event: string) => void;
export const reply: (id: number, ok: boolean, text: string, data: ArrayBuffer) => void;
export const open: (name: string, data: ArrayBuffer) => void;
export const active: (active: boolean) => void;
export const close: () => void;
export const command: (command: string) => void;
