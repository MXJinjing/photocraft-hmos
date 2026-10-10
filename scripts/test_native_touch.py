#!/usr/bin/env python3
"""Execute the actual native Touch/Axis callbacks with SDK-shaped events on the host."""
import json
from pathlib import Path
import subprocess
import tempfile
root = Path(__file__).resolve().parents[1]
source = (root / 'entry/src/main/cpp/bridge.cpp').read_text()
touch = source[source.index('void Touch('):source.index('void Mouse(')]
axis = source[source.index('void Axis('):source.index('void Blur(')]
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
}
'''
with tempfile.TemporaryDirectory() as tmp:
    cpp = Path(tmp)/'test.cpp';exe=Path(tmp)/'test'
    cpp.write_text(stubs+touch+axis+main)
    subprocess.run(['clang++','-std=c++17','-Wall','-Wextra','-Werror',str(cpp),'-o',str(exe)],check=True)
    packets=[json.loads(line) for line in subprocess.check_output([str(exe)],text=True).splitlines()]
assert [len(p['points']) for p in packets[:5]] == [1,2,2,1,0]
assert packets[2]['points']==[{'id':1,'x':20,'y':30},{'id':2,'x':40,'y':30}]
assert packets[5]['source']=='pen' and packets[5]['sample']['pressure']==.25
assert packets[6]['action']==1
assert packets[7]=={'kind':'axis','action':2,'x':100,'y':200,'dx':-4,'dy':-8,'zoom':1.2}
assert packets[8]['zoom']==0 and len(packets)==9
print('Passed: native multi-contact snapshots, release, pen priority, trackpad pinch/pan and anchor.')
