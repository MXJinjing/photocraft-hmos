#!/usr/bin/env python3
"""Execute native Touch/Mouse/Axis/Hover callbacks with SDK-shaped events on the host."""
import json
from pathlib import Path
import subprocess
import tempfile
root = Path(__file__).resolve().parents[1]
source = (root / 'entry/src/main/cpp/bridge.cpp').read_text()
touch = source[source.index('void Touch('):source.index('void Mouse(')]
mouse = source[source.index('void Mouse('):source.index('bool Key(')]
axis = source[source.index('void Axis('):source.index('void Blur(')]
hover = source[source.index('void Hover('):source.index('OH_NativeXComponent_Callback callbacks')]
stubs = r'''
#include <algorithm>
#include <atomic>
#include <cmath>
#include <iostream>
#include <string>
#include <vector>
enum OH_NativeXComponent_TouchPointToolType { OH_NATIVEXCOMPONENT_TOOL_TYPE_FINGER, OH_NATIVEXCOMPONENT_TOOL_TYPE_PEN, OH_NATIVEXCOMPONENT_TOOL_TYPE_RUBBER, OH_NATIVEXCOMPONENT_TOOL_TYPE_BRUSH, OH_NATIVEXCOMPONENT_TOOL_TYPE_PENCIL, OH_NATIVEXCOMPONENT_TOOL_TYPE_AIRBRUSH };
enum { OH_NATIVEXCOMPONENT_DOWN, OH_NATIVEXCOMPONENT_UP, OH_NATIVEXCOMPONENT_MOVE, OH_NATIVEXCOMPONENT_CANCEL, OH_NATIVEXCOMPONENT_RESULT_SUCCESS=0, OH_NATIVE_XCOMPONENT_MAX_TOUCH_POINTS_NUMBER=10 };
struct Point {int id;float x,y;bool isPressed;int type=OH_NATIVEXCOMPONENT_DOWN;};
struct OH_NativeXComponent_TouchEvent {int id,type;unsigned numPoints;float x,y,force;Point touchPoints[10];};
struct OH_NativeXComponent {};
OH_NativeXComponent_TouchEvent current{};
OH_NativeXComponent_TouchPointToolType tools[10]{};
int OH_NativeXComponent_GetTouchEvent(OH_NativeXComponent*,void*,OH_NativeXComponent_TouchEvent* e){*e=current;return 0;}
int OH_NativeXComponent_GetTouchPointToolType(OH_NativeXComponent*,unsigned i,OH_NativeXComponent_TouchPointToolType* t){*t=tools[i];return 0;}
int OH_NativeXComponent_GetTouchPointTiltX(OH_NativeXComponent*,unsigned,float* v){*v=0;return 0;}
int OH_NativeXComponent_GetTouchPointTiltY(OH_NativeXComponent*,unsigned,float* v){*v=0;return 0;}
enum { OH_NATIVEXCOMPONENT_MOUSE_NONE, OH_NATIVEXCOMPONENT_MOUSE_PRESS, OH_NATIVEXCOMPONENT_MOUSE_RELEASE, OH_NATIVEXCOMPONENT_MOUSE_MOVE, OH_NATIVEXCOMPONENT_MOUSE_CANCEL, OH_NATIVEXCOMPONENT_RIGHT_BUTTON=10, OH_NATIVEXCOMPONENT_MIDDLE_BUTTON=11 };
struct OH_NativeXComponent_MouseEvent { int action,button;float x,y; };
OH_NativeXComponent_MouseEvent currentMouse{};
int OH_NativeXComponent_GetMouseEvent(OH_NativeXComponent*,void*,OH_NativeXComponent_MouseEvent* e){*e=currentMouse;return 0;}
std::atomic<int> touchId(-1);
std::atomic<bool> penContact(false),fingerContact(false);
void Json(const std::string& s){std::cout<<s<<'\n';}
enum ArkUI_UIInputEvent_Type {ARKUI_UIINPUTEVENT_TYPE_AXIS};
struct ArkUI_UIInputEvent {double dx,dy,zoom;float x,y;int action;};
double OH_ArkUI_AxisEvent_GetHorizontalAxisValue(ArkUI_UIInputEvent* e){return e->dx;}
double OH_ArkUI_AxisEvent_GetVerticalAxisValue(ArkUI_UIInputEvent* e){return e->dy;}
double OH_ArkUI_AxisEvent_GetPinchAxisScaleValue(ArkUI_UIInputEvent* e){return e->zoom;}
float OH_ArkUI_PointerEvent_GetX(ArkUI_UIInputEvent* e){return e->x;}
float OH_ArkUI_PointerEvent_GetY(ArkUI_UIInputEvent* e){return e->y;}
int OH_ArkUI_AxisEvent_GetAxisAction(ArkUI_UIInputEvent* e){return e->action;}
'''
main = r'''
int main(){
    OH_NativeXComponent c;
    current={1,OH_NATIVEXCOMPONENT_DOWN,1,10,20,1,{{1,10,20,true}}};Touch(&c,nullptr);
    current={2,OH_NATIVEXCOMPONENT_DOWN,2,30,20,1,{{1,10,20,true},{2,30,20,true}}};Touch(&c,nullptr);
    current={2,OH_NATIVEXCOMPONENT_MOVE,2,40,30,1,{{1,20,30,false,OH_NATIVEXCOMPONENT_MOVE},{2,40,30,false,OH_NATIVEXCOMPONENT_MOVE}}};Touch(&c,nullptr);
    current.type=OH_NATIVEXCOMPONENT_UP;current.touchPoints[1].isPressed=false;Touch(&c,nullptr);
    current={1,OH_NATIVEXCOMPONENT_UP,1,20,30,1,{{1,20,30,false}}};Touch(&c,nullptr);
    current={3,OH_NATIVEXCOMPONENT_DOWN,1,40,50,.25,{{3,40,50,true}}};tools[0]=OH_NATIVEXCOMPONENT_TOOL_TYPE_PEN;Touch(&c,nullptr);
    ArkUI_UIInputEvent a{4,8,1.2,100,200,2};Axis(&c,&a,ARKUI_UIINPUTEVENT_TYPE_AXIS); // ignored while pen is down
    current.type=OH_NATIVEXCOMPONENT_UP;Touch(&c,nullptr);
    Axis(&c,&a,ARKUI_UIINPUTEVENT_TYPE_AXIS);
    a.zoom=0;Axis(&c,&a,ARKUI_UIINPUTEVENT_TYPE_AXIS);
    Hover(&c,true);Hover(&c,false);
    penContact=true;Hover(&c,true);Hover(&c,false);
    penContact=false;fingerContact=true;Hover(&c,true);Hover(&c,false);
    fingerContact=false;
    // A hover transition can dispatch the cached mouse event with action NONE.
    // It must not resurrect a brush tip at the pre-hover mouse position.
    currentMouse={OH_NATIVEXCOMPONENT_MOUSE_NONE,0,100,200};Mouse(&c,nullptr);
    currentMouse.action=OH_NATIVEXCOMPONENT_MOUSE_MOVE;Mouse(&c,nullptr);
    currentMouse.action=OH_NATIVEXCOMPONENT_MOUSE_PRESS;Mouse(&c,nullptr);
    currentMouse.action=OH_NATIVEXCOMPONENT_MOUSE_RELEASE;Mouse(&c,nullptr);
    currentMouse.action=OH_NATIVEXCOMPONENT_MOUSE_CANCEL;Mouse(&c,nullptr);
    // Lift finger 1 first, then move finger 2 while still pressed before its UP.
    tools[0]=tools[1]=OH_NATIVEXCOMPONENT_TOOL_TYPE_FINGER;
    current={1,OH_NATIVEXCOMPONENT_DOWN,2,100,200,1,{{1,100,200,true},{2,300,200,true}}};Touch(&c,nullptr);
    current={1,OH_NATIVEXCOMPONENT_UP,2,100,200,1,{{1,100,200,false,OH_NATIVEXCOMPONENT_UP},{2,300,200,false,OH_NATIVEXCOMPONENT_MOVE}}};Touch(&c,nullptr);
    current={2,OH_NATIVEXCOMPONENT_MOVE,1,320,210,1,{{2,320,210,false,OH_NATIVEXCOMPONENT_MOVE}}};Touch(&c,nullptr);
    current.x=340;current.touchPoints[0].x=340;Touch(&c,nullptr);
    current.type=OH_NATIVEXCOMPONENT_UP;Touch(&c,nullptr);
}
'''
with tempfile.TemporaryDirectory() as tmp:
    cpp = Path(tmp)/'test.cpp';exe=Path(tmp)/'test'
    cpp.write_text(stubs+touch+mouse+axis+hover+main)
    subprocess.run(['clang++','-std=c++17','-Wall','-Wextra','-Werror',str(cpp),'-o',str(exe)],check=True)
    packets=[json.loads(line) for line in subprocess.check_output([str(exe)],text=True).splitlines()]
assert [len(p['points']) for p in packets[:5]] == [1,2,2,1,0]
assert packets[2]['points']==[{'id':1,'x':20,'y':30},{'id':2,'x':40,'y':30}]
assert packets[5]['source']=='pen' and packets[5]['sample']['pressure']==.25
assert packets[6]['action']==1
assert packets[7]=={'kind':'axis','action':2,'x':100,'y':200,'dx':-4,'dy':-8,'zoom':1.2}
assert packets[8]['zoom']==0
assert packets[9]=={'kind':'hover_leave'}
assert packets[10:14]==[
    {'kind':'pointer','action':2,'button':0,'x':100,'y':200},
    {'kind':'pointer','action':0,'button':0,'x':100,'y':200},
    {'kind':'pointer','action':1,'button':0,'x':100,'y':200},
    {'kind':'pointer','action':3},
]
assert [len(p['points']) for p in packets[-5:]] == [2,1,1,1,0]
assert packets[-3]['points'] == [{'id':2,'x':320,'y':210}]
assert packets[-2]['points'] == [{'id':2,'x':340,'y':210}]
print('Passed: native contact, navigation, proximity leave and genuine mouse events; cached NONE events ignored.')
