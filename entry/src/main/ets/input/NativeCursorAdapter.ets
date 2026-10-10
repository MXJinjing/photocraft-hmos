import { pointer } from '@kit.InputKit';
import { UIContext } from '@kit.ArkUI';
import { BusinessError } from '@kit.BasicServicesKit';

export function restorePointerVisibility(): void {
  try { pointer.setPointerVisibleSync(true); }
  catch (error) { console.error(`Restore cursor: ${(error as BusinessError).message}`); }
}

function pointerStyle(icon: string): pointer.PointerStyle {
  switch (icon) {
    case 'Crosshair': case 'Cell': return pointer.PointerStyle.CROSS;
    case 'Text': return pointer.PointerStyle.TEXT_CURSOR;
    case 'VerticalText': return pointer.PointerStyle.HORIZONTAL_TEXT_CURSOR;
    case 'PointingHand': return pointer.PointerStyle.HAND_POINTING;
    case 'Grab': return pointer.PointerStyle.HAND_OPEN;
    case 'Grabbing': return pointer.PointerStyle.HAND_GRABBING;
    case 'Move': case 'AllScroll': return pointer.PointerStyle.MOVE;
    case 'ResizeHorizontal': case 'ResizeColumn': return pointer.PointerStyle.RESIZE_LEFT_RIGHT;
    case 'ResizeVertical': case 'ResizeRow': return pointer.PointerStyle.RESIZE_UP_DOWN;
    case 'ResizeNeSw': return pointer.PointerStyle.NORTH_EAST_SOUTH_WEST;
    case 'ResizeNwSe': return pointer.PointerStyle.NORTH_WEST_SOUTH_EAST;
    case 'ResizeEast': return pointer.PointerStyle.EAST;
    case 'ResizeWest': return pointer.PointerStyle.WEST;
    case 'ResizeNorth': return pointer.PointerStyle.NORTH;
    case 'ResizeSouth': return pointer.PointerStyle.SOUTH;
    case 'ResizeNorthEast': return pointer.PointerStyle.NORTH_EAST;
    case 'ResizeNorthWest': return pointer.PointerStyle.NORTH_WEST;
    case 'ResizeSouthEast': return pointer.PointerStyle.SOUTH_EAST;
    case 'ResizeSouthWest': return pointer.PointerStyle.SOUTH_WEST;
    case 'ZoomIn': return pointer.PointerStyle.ZOOM_IN;
    case 'ZoomOut': return pointer.PointerStyle.ZOOM_OUT;
    case 'Copy': return pointer.PointerStyle.CURSOR_COPY;
    case 'NoDrop': case 'NotAllowed': return pointer.PointerStyle.CURSOR_FORBID;
    case 'Help': return pointer.PointerStyle.HELP;
    case 'Wait': return pointer.PointerStyle.LOADING;
    case 'Progress': return pointer.PointerStyle.RUNNING;
    default: return pointer.PointerStyle.DEFAULT;
  }
}

export class NativeCursorAdapter {
  private context: UIContext;
  constructor(context: UIContext) { this.context = context; }

  update(icon: string): void {
    try {
      if (icon === 'None') {
        // egui has already drawn the brush tip or custom tool cursor.
        pointer.setPointerVisibleSync(false);
      } else {
        const controller = this.context.getCursorController();
        if (icon === 'Default') { controller.restoreDefault(); }
        else { controller.setCursor(pointerStyle(icon)); }
        pointer.setPointerVisibleSync(true);
      }
    } catch (error) {
      restorePointerVisibility();
      console.error(`Native cursor: ${(error as BusinessError).message}`);
    }
  }

  reset(): void {
    restorePointerVisibility();
    try { this.context.getCursorController().restoreDefault(); }
    catch (error) { console.error(`Reset cursor: ${(error as BusinessError).message}`); }
  }
}
