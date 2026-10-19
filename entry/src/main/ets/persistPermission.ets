import { fileShare } from '@kit.CoreFileKit';
import { BusinessError } from '@kit.BasicServicesKit';

/** SystemCapability for fileShare persist / activate / revoke (API 12+). */
export const FOLDER_AUTHORIZATION: string =
  'SystemCapability.FileManagement.AppFileService.FolderAuthorization';

export function persistCapable(): boolean {
  return canIUse(FOLDER_AUTHORIZATION);
}

export function isFileUri(uri: string): boolean {
  return uri.startsWith('file://') && uri.length > 'file://'.length;
}

function policy(uri: string, write: boolean): fileShare.PolicyInfo {
  let mode: number = fileShare.OperationMode.READ_MODE;
  if (write) {
    mode = fileShare.OperationMode.READ_MODE | fileShare.OperationMode.WRITE_MODE;
  }
  return { uri: uri, operationMode: mode } as fileShare.PolicyInfo;
}

/**
 * Persist a Picker-granted temporary URI so it can be re-activated after restart.
 * Returns false when the capability is missing or persist fails (caller keeps ephemeral behavior).
 */
export async function persistUri(uri: string, write: boolean): Promise<boolean> {
  if (!persistCapable() || !isFileUri(uri)) {
    return false;
  }
  try {
    await fileShare.persistPermission([policy(uri, write)]);
    return true;
  } catch (error) {
    let err = error as BusinessError;
    console.error(`persistPermission failed: ${err.code} ${err.message}`);
    return false;
  }
}

/**
 * Activate a previously persisted URI grant (required after every app / device restart).
 * If the system reports the URI was never persisted, try persist then activate once.
 */
export async function activateUri(uri: string, write: boolean): Promise<boolean> {
  if (!persistCapable() || !isFileUri(uri)) {
    return false;
  }
  let policies: Array<fileShare.PolicyInfo> = [policy(uri, write)];
  try {
    await fileShare.activatePermission(policies);
    return true;
  } catch (error) {
    let err = error as BusinessError<Array<fileShare.PolicyErrorResult>>;
    console.error(`activatePermission failed: ${err.code} ${err.message}`);
    let needsPersist = false;
    if (err.code === 13900001 && err.data !== undefined) {
      for (let i = 0; i < err.data.length; i++) {
        if (err.data[i].code === fileShare.PolicyErrorCode.PERMISSION_NOT_PERSISTED) {
          needsPersist = true;
        }
      }
    }
    if (!needsPersist) {
      return false;
    }
    if (!(await persistUri(uri, write))) {
      return false;
    }
    try {
      await fileShare.activatePermission(policies);
      return true;
    } catch (retryError) {
      let retry = retryError as BusinessError;
      console.error(`activatePermission retry failed: ${retry.code} ${retry.message}`);
      return false;
    }
  }
}
